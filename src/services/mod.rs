pub mod cloudflare;
pub mod dnsomatic;
pub mod duckdns;
pub mod dummy;
pub mod dynu;
pub mod ipv64;
pub mod noip;
pub mod porkbun;
pub mod router_1;
pub mod selfhost;
pub mod shared_dyndns;

use std::net::IpAddr;

use thiserror::Error;

use crate::util::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Suspension {
    // If the number of cycles is zero, the service proceeds as normal
    Cycles(u32),

    // Once suspended, the service is not updated until end of program
    Indefinite,
}

impl std::fmt::Display for Suspension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Suspension::Cycles(u) => write!(f, "{} cycle(s) left", u),
            Suspension::Indefinite => write!(f, "indefinitely"),
        }
    }
}

#[derive(Clone, Error, Debug)]
pub enum DdnsUpdateError {
    // used when CF really returned an error
    #[error("Cloudflare returned error code {0} \"{1}\"")]
    Cloudflare(u32, Box<str>),
    // used when a service says it succeeded, but the returned JSON is nonsense
    #[error("received erroneous JSON: {0}")]
    Json(Box<str>),

    #[error("DuckDNS rejected the request - check again your tokens and domains")]
    DuckDns,

    #[error("{0} returned error: {1}")]
    DynDns(&'static str, Box<str>),

    #[error("Porkbun returned error: {0}")]
    Porkbun(Box<str>),

    #[error("Router returned error: HTTP {0} - {1}")]
    Router1(u16, Box<str>),

    #[error("the daemon has suspended updating this service ({0})")]
    Suspended(Suspension),

    #[error("HTTP transport error: {0}")]
    TransportError(Box<str>),
}

pub trait DdnsService {
    /// Update the DNS records with the given IP addresses. If the update succeeds,
    /// one or two IP addresses (one for IPv4 and one for IPv6) will be returned.
    /// This does mean that it is not possible to set more than one IPv4/IPv6
    /// address for a given domain, but many DDNS services already don't support
    /// that.
    fn update_record(&mut self, ip: &[IpAddr]) -> Result<FixedVec<IpAddr, 2>, DdnsUpdateError>;
}
