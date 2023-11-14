use std::net::IpAddr;

use serde_derive::{Deserialize, Serialize};
use ureq::Error;

use crate::util::{one_or_more_string, FixedVec};
use crate::GENERAL_CONFIG;

use super::{DdnsService, DdnsUpdateError};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    username: Box<str>,
    password: Box<str>,

    #[serde(deserialize_with = "one_or_more_string")]
    domains: Vec<Box<str>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    config: Config,

    auth: Box<str>,
}

impl Service {
    pub fn from_config(config: Config) -> Self {
        let username_password = String::from(config.username.clone()) + ":" + &config.password;
        let base64 = data_encoding::BASE64.encode(username_password.as_bytes());
        let auth = String::from("Basic ") + &base64;

        Self {
            config,
            auth: auth.into(),
        }
    }
}

impl DdnsService for Service {
    fn update_record(&mut self, ips: &[IpAddr]) -> Result<FixedVec<IpAddr, 2>, DdnsUpdateError> {
        let ipv4 = ips.iter().find(|ip| ip.is_ipv4());
        let ipv6 = ips.iter().find(|ip| ip.is_ipv6());

        let mut result = FixedVec::new();

        let mut request = ureq::get("https://dynupdate.no-ip.com/nic/update")
            .set("Authorization", &self.auth)
            .set("User-Agent", &GENERAL_CONFIG.get().unwrap().user_agent)
            .query("hostname", &self.config.domains.join(","));

        if ipv4.is_some() && ipv6.is_some() {
            let myip = ipv4.unwrap().to_string() + "," + &ipv6.unwrap().to_string();
            request = request.query("myip", &myip);

            result.push(*ipv4.unwrap());
            result.push(*ipv6.unwrap());
        } else if ipv4.is_some() {
            request = request.query("myip", &ipv4.unwrap().to_string());
            result.push(*ipv4.unwrap());
        } else if ipv6.is_some() {
            request = request.query("myip", &ipv6.unwrap().to_string());
            result.push(*ipv6.unwrap());
        }

        match request.call() {
            Ok(resp) => {
                let resp = resp
                    .into_string()
                    .map_err(|e| DdnsUpdateError::NoIp(e.to_string().into()))?;

                if resp.starts_with("good") {
                    return Ok(result);
                } else if resp.starts_with("nochg") {
                    return Ok(FixedVec::new());
                }
            }
            Err(Error::Status(code, resp)) => {
                if code >= 500 {
                    Err(DdnsUpdateError::NoIp("NoIP server is down".into()))?
                } else if code >= 400 {
                    let resp = resp
                        .into_string()
                        .map_err(|e| DdnsUpdateError::NoIp(e.to_string().into()))?;

                    Err(DdnsUpdateError::NoIp(resp.into()))?
                } else {
                    unreachable!()
                }
            }
            Err(Error::Transport(t)) => Err(DdnsUpdateError::TransportError(t.to_string().into()))?,
        };

        Ok(result)
    }
}
