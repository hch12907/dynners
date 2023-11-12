pub mod cloudflare;

use std::net::IpAddr;

use thiserror::Error;

use crate::util::*;

#[derive(Clone, Error, Debug)]
pub enum DdnsUpdateError {
    // used when CF really returned an error
    #[error("Cloudflare returned error code {0} \"{1}\"")]
    Cloudflare(u32, Box<str>),
    // used when CF says it succeeded, but the returned JSON is nonsense
    #[error("Cloudflare returned erroneous JSON: {0}")]
    CloudflareJson(Box<str>),

    #[error("HTTP transport error: {0}")]
    TransportError(Box<str>),
}

pub trait DdnsService {
    fn update_record(&mut self, ip: &IpAddr) -> Result<(), DdnsUpdateError>;
}
