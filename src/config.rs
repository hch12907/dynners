use std::collections::HashMap;
use std::num::NonZeroU32;

use serde_derive::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::services::*;
use crate::util::{one_or_more_string, parse_number_into_optional_nonzero};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct General {
    #[serde(deserialize_with = "parse_number_into_optional_nonzero")]
    pub update_rate: Option<NonZeroU32>,
    #[serde(default = "default_shell")]
    pub shell: Box<str>,
    #[serde(default = "default_user_agent")]
    pub user_agent: Box<str>,
    #[serde(default = "default_persistent_state")]
    pub persistent_state: Box<str>,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "method")]
#[serde(rename_all = "lowercase")]
pub enum IpConfigMethod {
    Exec {
        command: Box<str>,
    },

    Interface {
        iface: Box<str>,

        #[serde(default)]
        matches: Box<str>,
    },

    Http {
        url: Box<str>,

        #[serde(default = "default_regex")]
        regex: Box<str>,
    },
}

#[derive(Deserialize_repr, Serialize_repr, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum IpVersion {
    V4 = 4,
    V6 = 6,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct IpConfig {
    pub version: IpVersion,
    #[serde(flatten)]
    pub method: IpConfigMethod,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "service")]
#[serde(rename_all = "kebab-case")]
pub enum DdnsConfigService {
    CloudflareV4(cloudflare::Config),
    DnsOMatic(dnsomatic::Config),
    Duckdns(duckdns::Config),
    Dynu(dynu::Config),
    Ipv64(dynu::Config),
    Linode(linode::Config),
    PorkbunV3(porkbun::Config),
    Selfhost(dynu::Config),
    NoIp(noip::Config),
    Dummy(dummy::Config),
}

impl DdnsConfigService {
    pub fn into_boxed(self) -> Box<dyn DdnsService> {
        match self {
            DdnsConfigService::CloudflareV4(cf) => Box::new(cloudflare::Service::from(cf)),

            DdnsConfigService::NoIp(np) => Box::new(noip::Service::from(np)),

            DdnsConfigService::DnsOMatic(dom) => Box::new(dnsomatic::Service::from(dom)),

            DdnsConfigService::Duckdns(dk) => Box::new(duckdns::Service::from(dk)),

            DdnsConfigService::Dynu(du) => Box::new(dynu::Service::from(du)),

            DdnsConfigService::Ipv64(ip) => Box::new(ipv64::Service::from(ip)),

            DdnsConfigService::Linode(li) => Box::new(linode::Service::from(li)),

            DdnsConfigService::PorkbunV3(pb) => Box::new(porkbun::Service::from(pb)),

            DdnsConfigService::Selfhost(sh) => Box::new(selfhost::Service::from(sh)),

            DdnsConfigService::Dummy(dm) => Box::new(dummy::Service::from(dm)),
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct DdnsConfig {
    #[serde(deserialize_with = "one_or_more_string")]
    pub ip: Vec<Box<str>>,

    #[serde(flatten)]
    pub service: DdnsConfigService,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub general: General,
    pub ip: HashMap<Box<str>, IpConfig>,
    pub ddns: HashMap<Box<str>, DdnsConfig>,
}

fn default_user_agent() -> Box<str> {
    concat!("github.com/hch12907/dynners ", env!("CARGO_PKG_VERSION")).into()
}

fn default_shell() -> Box<str> {
    "/bin/bash".into()
}

fn default_regex() -> Box<str> {
    "(.*)".into()
}

fn default_persistent_state() -> Box<str> {
    "/var/lib/dynners/persistence".into()
}
