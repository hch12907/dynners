#[cfg(feature = "curl")]
#[cfg(feature = "ureq")]
#[error("The features `curl` and `ureq` must not be enabled together!")]
const FORCED_ERROR: u8 = 1 / 0;

#[cfg(feature = "curl")]
mod curl_backend;

#[cfg(feature = "ureq")]
mod ureq_backend;

use std::io::{self, Read};

use serde::de::DeserializeOwned;

#[cfg(feature = "curl")]
pub use curl_backend::Request;

#[cfg(feature = "ureq")]
pub use ureq_backend::Request;

pub struct Response {
    pub(self) reader: Box<dyn Read>,
}

pub enum Error {
    Status(u16, Response),
    Transport(Box<str>),
}

impl Response {
    pub fn into_json<T: DeserializeOwned>(self) -> Result<T, io::Error> {
        serde_json::from_reader(self.reader)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn into_string(self) -> Result<String, io::Error> {
        let mut vec = Vec::with_capacity(1024);
        let read = self.reader.take(2 * 1024 * 1024).read_to_end(&mut vec)?;
        vec.resize(read, 0);
        String::from_utf8(vec).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }
}
