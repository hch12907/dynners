use std::net::IpAddr;

use serde_derive::{Deserialize, Serialize};

use crate::util::{one_or_more_string, FixedVec};

use super::{DdnsService, DdnsUpdateError};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    #[serde(deserialize_with = "one_or_more_string")]
    domains: Vec<Box<str>>,
}

pub struct Service {
    config: Config,
}

impl Service {
    pub fn from_config(config: Config) -> Self {
        Self { config }
    }
}

impl DdnsService for Service {
    fn update_record(&mut self, ips: &[IpAddr]) -> Result<FixedVec<IpAddr, 2>, DdnsUpdateError> {
        let ipv4 = ips.iter().find(|ip| ip.is_ipv4());
        let ipv4_str = ipv4.map(|ip| ip.to_string()).unwrap_or_default();
        let ipv6 = ips.iter().find(|ip| ip.is_ipv6());
        let ipv6_str = ipv6.map(|ip| ip.to_string()).unwrap_or_default();

        // Simulate updating the domains
        print!("Dummy: simulate updating the following domains: ");
        println!("{}", self.config.domains.join(", "));
        println!("... using the IP addresses: {} {}", ipv4_str, ipv6_str);

        // We return the addresses we use to update the DDNS back to main()
        let mut result = FixedVec::new();
        if ipv4.is_some() {
            result.push(*ipv4.unwrap());
        }
        if ipv6.is_some() {
            result.push(*ipv6.unwrap());
        }

        Ok(result)
    }
}
