use std::net::AddrParseError;
use std::str::FromStr;

#[cfg(feature = "regex")]
use regex::Regex;

use crate::http::{Error, Request};

pub(super) fn get_address<T>(
    url: &str,
    #[cfg(feature = "regex")] regex: &Regex,
) -> Result<T, String>
where
    T: FromStr<Err = AddrParseError>,
{
    let response = match Request::get(url).call() {
        Ok(r) => r,
        Err(Error::Status(code, response)) => {
            Err(code.to_string() + &response.into_string().unwrap_or_default())?
        }
        Err(Error::Transport(t)) => Err(t.to_string())?,
    };

    let text = response.into_string().map_err(|e| e.to_string())?;

    #[cfg(feature = "regex")]
    let addr = regex
        .captures(text.as_str())
        .and_then(|captured| captured.get(1))
        .map(|matched| matched.as_str().to_owned())
        .ok_or_else(|| {
            String::from("the following HTTP response does not match regex: ") + &text
        })?;

    #[cfg(not(feature = "regex"))]
    let addr = text.trim();

    addr.parse::<T>().map_err(|e| e.to_string())
}
