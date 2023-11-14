use std::net::{Ipv4Addr, Ipv6Addr};

use regex::Regex;
use ureq::Error;

use crate::USER_AGENT;

pub(super) fn get_address_v4(url: &str, regex: &Regex) -> Result<Ipv4Addr, String> {
    let response = match ureq::get(url)
        .set("User-Agent", USER_AGENT.get().unwrap())
        .call()
    {
        Ok(r) => r,
        Err(Error::Status(code, response)) => {
            Err(code.to_string() + &response.into_string().unwrap_or_default())?
        }
        Err(Error::Transport(t)) => Err(t.to_string())?,
    };

    let text = response.into_string().map_err(|e| e.to_string())?;

    let addr = regex
        .captures(text.as_str())
        .and_then(|captured| captured.get(1))
        .map(|matched| matched.as_str().to_owned())
        .ok_or_else(|| {
            String::from("the following HTTP response does not match regex: ") + &text
        })?;

    addr.parse::<Ipv4Addr>().map_err(|e| e.to_string())
}

pub(super) fn get_address_v6(url: &str, regex: &Regex) -> Result<Ipv6Addr, String> {
    let response = match ureq::get(url)
        .set("User-Agent", USER_AGENT.get().unwrap())
        .call()
    {
        Ok(r) => r,
        Err(Error::Status(code, response)) => {
            Err(code.to_string() + &response.into_string().unwrap_or_default())?
        }
        Err(Error::Transport(t)) => Err(t.to_string())?,
    };

    let text = response.into_string().map_err(|e| e.to_string())?;

    let addr = regex
        .captures(text.as_str())
        .and_then(|captured| captured.get(1))
        .map(|matched| matched.as_str().to_owned())
        .ok_or_else(|| {
            String::from("the following HTTP response does not match regex: ") + &text
        })?;

    addr.parse::<Ipv6Addr>().map_err(|e| e.to_string())
}
