use std::net::IpAddr;

use serde_derive::{Deserialize, Serialize};

use crate::http::{Error, Request};
use crate::util::{one_or_more_string, FixedVec};

use super::{DdnsService, DdnsUpdateError};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    token: Box<str>,

    #[serde(deserialize_with = "one_or_more_string")]
    domains: Vec<Box<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    config: Config,
}

impl From<Config> for Service {
    fn from(config: Config) -> Self {
        Self { config }
    }
}

impl DdnsService for Service {
    fn update_record(&mut self, ips: &[IpAddr]) -> Result<FixedVec<IpAddr, 2>, DdnsUpdateError> {
        let ipv4 = ips.iter().find(|ip| ip.is_ipv4());
        let ipv6 = ips.iter().find(|ip| ip.is_ipv6());

        let mut request = Request::get("https://www.duckdns.org/update")
            .query("domains", &self.config.domains.join(","))
            .query("token", &self.config.token);

        let mut result = FixedVec::new();

        if let Some(ipv4) = ipv4 {
            request = request.query("ip", &ipv4.to_string());
            result.push(*ipv4);
        }

        if let Some(ipv6) = ipv6 {
            request = request.query("ipv6", &ipv6.to_string());
            result.push(*ipv6);
        }

        match request.call() {
            Ok(resp) | Err(Error::Status(_, resp)) => {
                let resp = resp.into_string().map_err(|_| DdnsUpdateError::DuckDns)?;

                if resp.starts_with("OK") || resp.starts_with("good") {
                    Ok(result)
                } else if resp.starts_with("KO") {
                    Err(DdnsUpdateError::DuckDns)
                } else {
                    // According to the API documentation, the only possible responses
                    // (without setting verbose=true) are OK and KO. So theoretically
                    // we shouldn't reach this branch... but make an error if we do
                    Err(DdnsUpdateError::DuckDns)
                }
            }

            Err(Error::Transport(t)) => Err(DdnsUpdateError::TransportError(t.to_string().into()))?,
        }
    }
}
