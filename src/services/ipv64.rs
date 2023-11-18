use std::net::IpAddr;

use crate::util::FixedVec;

use super::{shared_dyndns, DdnsService, DdnsUpdateError};

pub type Config = shared_dyndns::Config;

pub struct Service {
    inner: shared_dyndns::Service,
}

impl From<Config> for Service {
    fn from(config: Config) -> Self {
        Self {
            inner: shared_dyndns::Service::from_config(
                "IPv64",
                "https://ipv64.net/nic/update",
                config,
            ),
        }
    }
}

impl DdnsService for Service {
    fn update_record(&mut self, ip: &[IpAddr]) -> Result<FixedVec<IpAddr, 2>, DdnsUpdateError> {
        self.inner.update_record(ip)
    }
}
