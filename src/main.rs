mod config;
mod http;
mod ip;
mod services;
mod persistence;
mod util;

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, BufReader, BufWriter};
use std::sync::OnceLock;
use std::time::Duration;

use config::{Config, General};
use persistence::PersistentState;

const CONFIG_PATHS: [&'static str; 2] = [
    "./config.toml",
    #[cfg(target_family = "unix")]
    "/etc/dynners/config.toml",
];

/// This stores config values specified inside the [general] section of
/// config.toml.
static GENERAL_CONFIG: OnceLock<General> = OnceLock::new();

fn check_curl_version() {
    #[cfg(feature = "curl")]
    {
        let num = curl::Version::get().version_num();
        let major = (num >> 16) & 0xFF;
        let minor = (num >> 8) & 0xFF;

        // As of writing, this is the oldest supported curl in Debian 10.
        // Not going to support anything older than that.
        if !(major > 7 || (major == 7 && minor >= 64)) {
            println!("System libcurl is too old! Minimum required: 7.64.0");
            std::process::exit(1);
        }

        if curl::Version::get().ssl_version().is_none() {
            println!("libcurl doesn't seem to have SSL support. Exiting.");
            std::process::exit(1);
        }
    }
}

fn main() {
    check_curl_version();

    let mut config_str = String::new();

    for path in CONFIG_PATHS {
        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };

        match file.read_to_string(&mut config_str) {
            Ok(_) => break,
            Err(e) => println!("Unable to read config file, reason: {}", e.to_string()),
        }
    }

    if config_str.is_empty() {
        println!("No configuration found. Quitting.");
        return;
    }

    // Calculating the hash of current config file
    let config_hash = PersistentState::new(&config_str).config_hash;

    // Parsing the config file
    let config = match toml::from_str::<Config>(config_str.as_str()) {
        Ok(conf) => conf,
        Err(e) => return println!("{}", e.to_string()),
    };

    // Reading and parsing the persistent state
    let mut persistent_state = 'block: {
        let file = match File::open(config.general.persistent_state.as_ref()) {
            Ok(f) => f,
            Err(_) => break 'block PersistentState::new(&config_str),
        };

        match PersistentState::from_reader(BufReader::new(file)) {
            Ok(state) => {
                println!("[INFO] Loaded persistent state.");
                state
            },

            Err(e) => {
                println!("[WARN] Couldn't read persistent state file, reason: {}", e.to_string());
                PersistentState::new(&config_str)
            },
        }
    };
    
    if !persistent_state.validate_against(&config_str) {
        println!("[INFO] Discarded the persistent state because config file has changed.")
    }

    let update_rate = config.general.update_rate;

    println!(
        "dynners v{} started, updating every {} second(s)",
        env!("CARGO_PKG_VERSION"),
        update_rate.map(|x| u32::from(x)).unwrap_or(0)
    );

    // It's safe to unwrap here - the program is single-threaded and USER_AGENT
    // is never initialized before reaching this point of program.
    GENERAL_CONFIG.set(config.general).unwrap();

    // Collect IP addresses specified in [ip.*] entries into (ip name, ip)
    let mut ips = HashMap::with_capacity(config.ip.len());
    for (name, ip) in config.ip.into_iter() {
        let mut dyn_ip = match ip::DynamicIp::from_config(&ip) {
            Ok(d) => d,
            Err(e) => return println!("Unable to parse IP configuration: {}", e.to_string()),
        };

        if let Some(ip) = persistent_state.ip_addresses.get(&name) {
            println!("[INFO] Initialized IP {} using the persistent state with {}", &name, &ip);
            dyn_ip.update_from_cache(ip.clone());
        }

        ips.insert(name, dyn_ip);
    }

    if ips.is_empty() {
        println!("No IPs were configured. Quitting.");
        return;
    }

    // Collect IP addresses specified in [ddns.*] entries into (ddns name, ip name)
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
    for (name, service_conf) in &config.ddns {
        let service = service_conf.service.clone().to_boxed();
        services.push((name, service))
    }

    // Main loop here
    loop {
        let mut is_ip_updated = false;

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

            is_ip_updated |= is_dirty;

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

        // We only update the persistent state if any of the IPs have changed.
        if is_ip_updated {
            persistent_state = PersistentState::new_with_config_hash(config_hash);
            persistent_state.ip_addresses = ips
                .iter()
                .flat_map(|(name, dyn_ip)|
                    dyn_ip.address().map(|ip| (name.clone(), ip.clone()))
                )
                .collect();

            let path = GENERAL_CONFIG.get().unwrap().persistent_state.as_ref();

            let file = match File::create(path) {
                Ok(f) => Some(f),
                Err(_) if path.is_empty() => None,
                Err(e) => {
                    println!("[WARN] Couldn't open persistent state file for writing: {}", e.to_string());
                    None
                }
            };

            if let Some(file) = file {
                match persistent_state.write_to(BufWriter::new(file)){
                    Ok(_) => (),
                    Err(e) => {
                        println!("[WARN] Couldn't write to persistent state file: {}", e.to_string());
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
