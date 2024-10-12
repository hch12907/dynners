use std::convert::Infallible;
use std::net::IpAddr;

use serde_derive::{Deserialize, Serialize};

use crate::http::{Error, Request};
use crate::util::{one_or_more_string, FixedVec};

use super::{DdnsService, DdnsUpdateError};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    secret_api_key: Box<str>,

    api_key: Box<str>,

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

impl Service {
    fn parse_error(error: Error) -> Result<Infallible, DdnsUpdateError> {
        match error {
            Error::Status(code, resp) if code < 500 => {
                let json = resp
                    .into_json::<serde_json::Value>()
                    .map_err(|e| DdnsUpdateError::Json(e.to_string().into()))?;
                let message = json
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("(null)");
                Err(DdnsUpdateError::Porkbun(message.to_owned().into()))
            }
            Error::Status(code, _resp) => {
                let message = code.to_string();
                Err(DdnsUpdateError::Porkbun(message.into()))
            }
            Error::Transport(t) => Err(DdnsUpdateError::TransportError(t.to_string().into())),
        }
    }
}

impl DdnsService for Service {
    fn update_record(&mut self, ips: &[IpAddr]) -> Result<FixedVec<IpAddr, 2>, DdnsUpdateError> {
        let ipv4 = ips.iter().find(|ip| ip.is_ipv4());
        let ipv6 = ips.iter().find(|ip| ip.is_ipv6());

        let mut ipv4_succeeded = false;
        let mut ipv6_succeeded = false;

        for domain in &self.config.domains {
            let subdomain_parts = domain.split('.').rev().skip(2).collect::<Vec<_>>();
            let subdomain = subdomain_parts
                .into_iter()
                .rfold(String::new(), |acc, x| acc + "." + x);
            let subdomain = subdomain.trim_start_matches('.');

            // UNWRAP-SAFETY: subdomain is guaranteed to be the prefix of domain
            let domain = domain
                .strip_prefix(subdomain)
                .unwrap()
                .trim_start_matches('.');

            if let Some(ipv4) = ipv4 {
                let url = format!(
                    "https://api.porkbun.com/api/json/v3/dns/editByNameType/{}/A/{}",
                    domain, subdomain
                );

                let request = Request::post(&url)
                    .send_json(serde_json::json!({
                        "secretapikey": &self.config.secret_api_key,
                        "apikey": &self.config.api_key,
                        "content": ipv4.to_string(),
                    }))
                    .map_err(|e| Self::parse_error(e).unwrap_err())?;

                let json = request
                    .into_json::<serde_json::Value>()
                    .map_err(|e| DdnsUpdateError::Json(e.to_string().into()))?;

                let success = json.get("status").and_then(|v| v.as_str()) == Some("SUCCESS");

                ipv4_succeeded |= success;
            }

            if let Some(ipv6) = ipv6 {
                let url = format!(
                    "https://api.porkbun.com/api/json/v3/dns/editByNameType/{}/AAAA/{}",
                    domain, subdomain
                );

                let request = Request::post(&url)
                    .send_json(serde_json::json!({
                        "secretapikey": &self.config.secret_api_key,
                        "apikey": &self.config.api_key,
                        "content": ipv6.to_string(),
                    }))
                    .map_err(|e| Self::parse_error(e).unwrap_err())?;

                let json = request
                    .into_json::<serde_json::Value>()
                    .map_err(|e| DdnsUpdateError::Json(e.to_string().into()))?;

                let success = json.get("status").and_then(|v| v.as_str()) == Some("SUCCESS");

                ipv6_succeeded |= success;
            }
        }

        let mut result = FixedVec::new();
        if ipv4_succeeded {
            // UNWRAP-SAFETY: ipv4 (and ipv6 below) can only succeed when
            // it is not None
            result.push(*ipv4.unwrap());
        }
        if ipv6_succeeded {
            result.push(*ipv6.unwrap());
        }

        Ok(result)
    }
}
