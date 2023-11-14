mod config;
mod ip;
mod services;
mod util;

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::sync::OnceLock;
use std::time::Duration;

use config::{Config, DdnsConfigService, General};
use services::DdnsService;

use crate::services::cloudflare;

const CONFIG_PATHS: [&'static str; 2] = [
    "./config.toml",
    #[cfg(target_family = "unix")]
    "/etc/config.toml",
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
                    continue;
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

        if let Some(sleep_for) = &update_rate {
            std::thread::sleep(Duration::from_secs(sleep_for.get() as u64));
        } else {
            break; // 0 timeout makes this a fire-once program.
        }
    }
}
