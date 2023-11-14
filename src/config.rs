use std::collections::HashMap;
use std::num::NonZeroU32;

use serde_derive::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

use crate::services;
use crate::util::one_or_more_string;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct General {
    pub update_rate: Option<NonZeroU32>,
    #[serde(default = "default_shell")]
    pub shell: Box<str>,
    #[serde(default)]
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
    NoIp(services::noip::Config),
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

fn default_shell() -> Box<str> {
    "/bin/bash".into()
}

#[cfg(feature = "regex")]
fn default_regex() -> Box<str> {
    "(.*)".into()
}
