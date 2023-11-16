mod config;
mod http;
mod ip;
mod services;
mod util;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::OnceLock;
use std::time::Duration;

use config::{Config, DdnsConfigService, General};
use services::*;

use crate::services::cloudflare;

const CONFIG_PATHS: [&'static str; 2] = [
    "./config.toml",
    #[cfg(target_family = "unix")]
    "/etc/dynners/config.toml",
];

static GENERAL_CONFIG: OnceLock<General> = OnceLock::new();

fn main() {
    let mut config = String::new();
    for path in CONFIG_PATHS {
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        match file.read_to_string(&mut config) {
            Ok(_) => break,
            Err(e) => println!("Unable to read config file, reason: {}", e.to_string()),
        }
    }

    if config.is_empty() {
        println!("No configuration found. Quitting.");
        return;
    }

    // Parsing the config file
    let config = match toml::from_str::<Config>(config.as_str()) {
        Ok(conf) => conf,
        Err(e) => return println!("{}", e.to_string()),
    };

    let update_rate = config.general.update_rate;

    // It's safe to unwrap here - the program is single-threaded and USER_AGENT
    // is never initialized before reaching this point of program.
    GENERAL_CONFIG.set(config.general).unwrap();

    // Collect IP addresses specified in [ip.*] entries into (ip name, ip)
    let mut ips = HashMap::with_capacity(config.ip.len());
    for (name, ip) in config.ip.into_iter() {
        let dyn_ip = match ip::DynamicIp::from_config(&ip) {
            Ok(d) => d,
            Err(e) => return println!("Unable to parse IP configuration: {}", e.to_string()),
        };

        ips.insert(name, dyn_ip);
    }

    if ips.is_empty() {
        println!("No IPs were configured. Quitting.");
        return;
    }

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
    let mut services = Vec::new();
    for (name, service) in &config.ddns {
        let service: Box<dyn DdnsService> = match &service.service {
            DdnsConfigService::CloudflareV4(cf) => {
                Box::new(cloudflare::Service::from_config(cf.clone()))
            }

            DdnsConfigService::NoIp(np) => Box::new(noip::Service::from_config(np.clone())),

            DdnsConfigService::DnsOMatic(dom) => {
                Box::new(dnsomatic::Service::from_config(dom.clone()))
            }

            DdnsConfigService::Duckdns(dk) => Box::new(duckdns::Service::from_config(dk.clone())),

            DdnsConfigService::Dynu(du) => Box::new(dynu::Service::from_config(du.clone())),

            DdnsConfigService::Ipv64(ip) => Box::new(ipv64::Service::from_config(ip.clone())),

            DdnsConfigService::Porkbun(pb) => Box::new(porkbun::Service::from_config(pb.clone())),

            DdnsConfigService::Selfhost(sh) => Box::new(selfhost::Service::from_config(sh.clone())),

            DdnsConfigService::Dummy(dm) => Box::new(dummy::Service::from_config(dm.clone())),
        };

        services.push((name, service))
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
            let is_dirty = service_ips[name]
                .iter()
                .map(|name| &ips[name])
                .any(|ip| ip.is_dirty());

            if !is_dirty {
                continue;
            }

            let ips = service_ips[name]
                .iter()
                .map(|name| &ips[name])
                .filter_map(|ip| ip.address())
                .cloned()
                .collect::<Vec<_>>(); // TODO: use collect_into in the future

            match service.update_record(ips.as_slice()) {
                Ok(updated) => {
                    for ip in updated.as_slice() {
                        println!("[INFO] Updated DDNS service {} with IP {}", name, ip);
                    }

                    if updated.get(0).is_none() {
                        println!(
                            "[INFO] Tried to update DDNS service {}, but no changes were made",
                            name
                        );
                    }
                }

                Err(e) => {
                    println!(
                        "[ERROR] DDNS service {} failed, reason: {}",
                        name,
                        e.to_string()
                    )
                }
            };
        }

        if let Some(sleep_for) = &update_rate {
            std::thread::sleep(Duration::from_secs(sleep_for.get() as u64));
        } else {
            break; // 0 timeout makes this a fire-once program.
        }
    }
}
