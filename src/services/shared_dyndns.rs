use std::net::IpAddr;

use serde_derive::{Deserialize, Serialize};

use crate::http::{Error, Request};
use crate::util::{one_or_more_string, FixedVec};
use crate::GENERAL_CONFIG;

use super::{DdnsService, DdnsUpdateError, Suspension};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    username: Box<str>,
    password: Box<str>,

    #[serde(deserialize_with = "one_or_more_string")]
    domains: Vec<Box<str>>,
}

/// This is a shared implementation for all services using DynDNS v2 as their
/// API. All services using this implementation must provide a `name` which is
/// human-readable (it shows up in the logs) and the URL to the `server`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    name: &'static str,
    server: &'static str,
    config: Config,
    suspended: Suspension,
    auth: Box<str>,
}

impl Service {
    pub fn from_config(name: &'static str, server: &'static str, config: Config) -> Self {
        let username_password = String::from(config.username.clone()) + ":" + &config.password;
        let base64 = data_encoding::BASE64.encode(username_password.as_bytes());
        let auth = String::from("Basic ") + &base64;

        Self {
            config,
            suspended: Suspension::Cycles(0),
            auth: auth.into(),
            name,
            server,
        }
    }
}

impl DdnsService for Service {
    fn update_record(&mut self, ips: &[IpAddr]) -> Result<FixedVec<IpAddr, 2>, DdnsUpdateError> {
        match &mut self.suspended {
            Suspension::Cycles(cycles) if *cycles > 0 => {
                *cycles -= 1;
                return Err(DdnsUpdateError::Suspended(self.suspended.clone()));
            }
            Suspension::Indefinite => {
                return Err(DdnsUpdateError::Suspended(self.suspended.clone()))
            }
            _ => (),
        }

        let ipv4 = ips.iter().find(|ip| ip.is_ipv4());
        let ipv6 = ips.iter().find(|ip| ip.is_ipv6());

        let request = Request::get(self.server)
            .set("Authorization", &self.auth)
            .query("hostname", &self.config.domains.join(","));

        let request = if ipv4.is_some() && ipv6.is_some() {
            let myip = ipv4.unwrap().to_string() + "," + &ipv6.unwrap().to_string();
            request.query("myip", &myip)
        } else if let Some(ipv4) = ipv4 {
            request.query("myip", &ipv4.to_string())
        } else if let Some(ipv6) = ipv6 {
            request.query("myip", &ipv6.to_string())
        } else {
            unreachable!()
        };

        let mut result = FixedVec::new();

        match request.call() {
            Ok(resp) | Err(Error::Status(_, resp)) => {
                let resp = resp
                    .into_string()
                    .map_err(|e| DdnsUpdateError::DynDns(self.name, e.to_string().into()))?;

                if let Some(resp) = resp.strip_prefix("good") {
                    let mut split = resp.split(',');

                    let mut ip1 = split.next().and_then(|r| r.trim().parse::<IpAddr>().ok());
                    let mut ip2 = split.next().and_then(|r| r.trim().parse::<IpAddr>().ok());

                    // Some DDNS services don't seem to return IPs even though
                    // "good" is returned. In that case, return all known IPs.
                    if ip1.is_none() && ip2.is_none() {
                        ip1 = ipv4.cloned();
                        ip2 = ipv6.cloned();
                    }

                    if let Some(ip) = ip1 {
                        result.push(ip);
                    }
                    if let Some(ip) = ip2 {
                        result.push(ip);
                    }

                    Ok(result)
                } else if resp.starts_with("nochg") {
                    Ok(FixedVec::new())
                } else if resp.starts_with("911") || resp.starts_with("dnserr") {
                    let update_rate = GENERAL_CONFIG.get().unwrap().update_rate;

                    // We have encountered a server error - best to stop updating
                    // for about 30 minutes.
                    let cycles = match update_rate {
                        Some(rate) => (30 * 60) / u32::from(rate),
                        None => 0, // doesn't matter anyway, the program dies after this
                    };

                    self.suspended = Suspension::Cycles(cycles);

                    let error_message = match cycles {
                        0 => String::from("The server is down"),
                        n => format!("The server is down, suspending for {} cycles", n),
                    };

                    Err(DdnsUpdateError::DynDns(self.name, error_message.into()))
                } else {
                    // The user has done something wrong (or we have done something
                    // wrong). Suspend the updating of this service indefinitely or
                    // we risk having our client / user agent banned.
                    self.suspended = Suspension::Indefinite;

                    let resp = if resp.starts_with("!donator") {
                        String::from("Only credited users are allowed")
                    } else if resp.starts_with("badauth") {
                        String::from("Bad authentication details were provided")
                    } else if resp.starts_with("notfqdn") {
                        String::from("Domain must be fully-qualified")
                    } else if resp.starts_with("nohost") {
                        String::from("Hostname does not exist in the user account")
                    } else if resp.starts_with("abuse") {
                        String::from("Domain is blocked because of abuse")
                    } else if resp.starts_with("numhost") {
                        String::from("Too many hosts are specified")
                    } else if resp.starts_with("badagent") {
                        String::from(concat!(
                            "Bad user agent was provided. ",
                            "Configure your user_agent properly in the config file."
                        ))
                    } else {
                        resp
                    };

                    Err(DdnsUpdateError::DynDns(self.name, resp.into()))
                }
            }

            Err(Error::Transport(t)) => Err(DdnsUpdateError::TransportError(t.to_string().into()))?,
        }
    }
}
