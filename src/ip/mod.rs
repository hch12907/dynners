mod http;
mod interface;
mod netmask;

use std::net::IpAddr;

use regex::Regex;
use thiserror::Error;

use crate::config::{IpConfig, IpConfigMethod, IpVersion};

use netmask::{NetworkParseErr, NetworkV4, NetworkV6};

#[derive(Debug, Clone)]
pub enum IpService {
    HttpV4 { url: Box<str>, regex: Regex },

    InterfaceV4 { iface: Box<str>, matches: NetworkV4 },

    HttpV6 { url: Box<str>, regex: Regex },

    InterfaceV6 { iface: Box<str>, matches: NetworkV6 },
}

#[derive(Debug)]
pub struct DynamicIp {
    address: Option<IpAddr>,
    dirty: bool,
    service: IpService,
}

#[derive(Debug, Error, Clone)]
pub enum DynamicIpError {
    #[error("unable to obtain matching IP from interface")]
    InterfaceFailure,

    #[error("unable to obtain matching IP using HTTP: {0}")]
    HttpFailure(Box<str>),

    #[error("unable to parse the regex: {0}")]
    InvalidRegex(regex::Error),

    #[error("unable to parse the netmask: {0}")]
    InvalidNetwork(NetworkParseErr),
}

impl IpService {
    fn from_config(config: &IpConfig) -> Result<Self, DynamicIpError> {
        match (&config.version, &config.method) {
            (IpVersion::V4, IpConfigMethod::Interface { iface, matches }) => {
                let matches = matches
                    .trim()
                    .parse::<NetworkV4>()
                    .map_err(|e| DynamicIpError::InvalidNetwork(e))?;
                Ok(Self::InterfaceV4 {
                    iface: iface.clone(),
                    matches,
                })
            }

            (IpVersion::V4, IpConfigMethod::Http { url, regex }) => {
                let regex =
                    Regex::new(regex.as_ref()).map_err(|e| DynamicIpError::InvalidRegex(e))?;

                Ok(Self::HttpV4 {
                    url: url.clone(),
                    regex,
                })
            }

            (IpVersion::V6, IpConfigMethod::Interface { iface, matches }) => {
                let matches = matches
                    .trim()
                    .parse::<NetworkV6>()
                    .map_err(|e| DynamicIpError::InvalidNetwork(e))?;
                Ok(Self::InterfaceV6 {
                    iface: iface.clone(),
                    matches,
                })
            }

            (IpVersion::V6, IpConfigMethod::Http { url, regex }) => {
                let regex =
                    Regex::new(regex.as_ref()).map_err(|e| DynamicIpError::InvalidRegex(e))?;

                Ok(Self::HttpV6 {
                    url: url.clone(),
                    regex,
                })
            }
        }
    }
}

impl DynamicIp {
    pub fn from_config(config: &IpConfig) -> Result<Self, DynamicIpError> {
        Ok(Self {
            address: None,
            dirty: true,
            service: IpService::from_config(config)?,
        })
    }

    pub fn address(&self) -> Option<&IpAddr> {
        self.address.as_ref()
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn update(&mut self) -> Result<(), DynamicIpError> {
        let new_ip = match self.service {
            IpService::InterfaceV4 {
                ref iface,
                ref matches,
            } => interface::get_interface_v4_addresses(iface, matches)
                .map(IpAddr::from)
                .ok_or(DynamicIpError::InterfaceFailure),

            IpService::HttpV4 { ref url, ref regex } => http::get_address_v4(url, regex)
                .map(IpAddr::from)
                .map_err(|e| DynamicIpError::HttpFailure(e.into())),

            IpService::InterfaceV6 {
                ref iface,
                ref matches,
            } => interface::get_interface_v6_addresses(iface, matches)
                .map(IpAddr::from)
                .ok_or(DynamicIpError::InterfaceFailure),

            IpService::HttpV6 { ref url, ref regex } => http::get_address_v6(url, regex)
                .map(IpAddr::from)
                .map_err(|e| DynamicIpError::HttpFailure(e.into())),
        }?;

        if let Some(old_ip) = &self.address {
            self.dirty = *old_ip != new_ip;
        }

        self.address = Some(new_ip);

        Ok(())
    }
}
