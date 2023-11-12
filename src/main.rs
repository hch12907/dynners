mod config;
mod ip;
mod services;
mod util;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::time::Duration;

use config::{Config, DdnsConfigService};
use services::DdnsService;

use crate::services::cloudflare;

fn main() {
    let mut file = File::open("config.toml").unwrap();
    let mut config = String::new();
    file.read_to_string(&mut config).unwrap();

    // Parsing the config file
    let config = match toml::from_str::<Config>(config.as_str()) {
        Ok(conf) => conf,
        Err(e) => return println!("{}", e.to_string()),
    };

    // Collect IP addresses specified in [ip.*] entries into (ip name, ip)
    let mut ips = config
        .ip
        .into_iter()
        .map(|(name, ip)| (name, ip::DynamicIp::from_config(&ip).unwrap()))
        .collect::<HashMap<_, _>>();

    // Collect IP addresses specified in [ddns.*] entries into (ddns name, ip)
    let service_ips = config
        .ddns
        .iter()
        .map(|(name, ddns)| (name, &ddns.ip))
        .collect::<HashMap<_, _>>();

    // Verify whether the IPs in [ddns.*] are actually specified by [ip.*]
    let mut errored = false;
    for (service_name, service_ips) in service_ips.iter() {
        for ip in service_ips.iter() {
            if ips.get(ip).is_none() {
                println!(
                    "[FATAL] service {}: the IP {} is not specified anywhere in config",
                    service_name, ip
                );
                errored = true
            }
        }
    }

    if errored {
        return;
    }

    // Initialize each DDNS service entry into a `services` array
    let mut services = Vec::<(&Box<str>, Box<dyn DdnsService>)>::new();
    for (name, service) in &config.ddns {
        match &service.service {
            DdnsConfigService::CloudflareV4(cf) => {
                let service = cloudflare::Service::from_config(cf.clone());
                services.push((name, Box::new(service)));
            }
        }
    }

    // Main loop here
    loop {
        for (name, ip) in &mut ips {
            if let Err(e) = ip.update() {
                println!(
                    "[ERROR] Unable to update IP {}, reason: {}",
                    name,
                    e.to_string()
                );
            }
        }

        for (name, service) in services.iter_mut() {
            for ip in service_ips[name] {
                if !ips[ip].is_dirty() {
                    continue
                }

                if let Some(addr) = ips[ip].address() {
                    if let Err(e) = service.update_record(addr) {
                        println!("[ERROR] DDNS service {} failed, reason: {}", name, e)
                    } else {
                        println!("[INFO] Updated DDNS service {} with IP {}", name, addr);
                        break;
                    }
                }
            }
        }

        if let Some(sleep_for) = &config.general.update_rate {
            std::thread::sleep(Duration::from_secs(sleep_for.get() as u64));
        } else {
            break // 0 timeout makes this a fire-once program.
        }
    }
}
