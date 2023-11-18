use std::collections::HashMap;
use std::num::NonZeroU32;

use serde_derive::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::services;
use crate::util::{one_or_more_string, parse_number_into_optional_nonzero};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct General {
    #[serde(deserialize_with = "parse_number_into_optional_nonzero")]
    pub update_rate: Option<NonZeroU32>,
    #[serde(default = "default_shell")]
    pub shell: Box<str>,
    #[serde(default = "default_user_agent")]
    pub user_agent: Box<str>,
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

        #[cfg(feature = "regex")]
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
    CloudflareV4(services::cloudflare::Config),
    DnsOMatic(services::dnsomatic::Config),
    Duckdns(services::duckdns::Config),
    Dynu(services::dynu::Config),
    Ipv64(services::dynu::Config),
    PorkbunV3(services::porkbun::Config),
    Selfhost(services::dynu::Config),
    NoIp(services::noip::Config),
    Dummy(services::dummy::Config),
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

#[cfg(feature = "regex")]
fn default_regex() -> Box<str> {
    "(.*)".into()
}
