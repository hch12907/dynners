use std::io::Cursor;

use curl::easy::{Easy, List};
use serde::Serialize;

use crate::GENERAL_CONFIG;

use super::{Error, Response};

pub struct Request {
    curl: Easy,
    header_list: List,
    url: Box<str>,
    queries: String,
}

impl Request {
    pub fn get(url: &str) -> Self {
        let mut curl = Easy::new();
        // UNWRAP-SAFETY: HTTP is supported. And we are already screwed if it isn't...
        curl.get(true).unwrap();
        curl.useragent(&GENERAL_CONFIG.get().unwrap().user_agent)
            .expect("out of memory");

        Self {
            curl,
            header_list: List::new(),
            url: url.into(),
            queries: String::new(),
        }
    }

    pub fn post(url: &str) -> Self {
        let mut curl = Easy::new();
        // UNWRAP-SAFETY: HTTP is supported.
        curl.post(true).unwrap();
        curl.useragent(&GENERAL_CONFIG.get().unwrap().user_agent)
            .expect("out of memory");

        Self {
            curl,
            header_list: List::new(),
            url: url.into(),
            queries: String::new(),
        }
    }

    pub fn put(url: &str) -> Self {
        let mut curl = Easy::new();
        // UNWRAP-SAFETY: HTTP is supported. And we are already screwed if it isn't...
        curl.put(true).unwrap();
        curl.useragent(&GENERAL_CONFIG.get().unwrap().user_agent)
            .expect("out of memory");

        Self {
            curl,
            header_list: List::new(),
            url: url.into(),
            queries: String::new(),
        }
    }

    pub fn query(mut self, param: &str, value: &str) -> Self {
        if self.queries.is_empty() {
            self.queries = self.queries + "?" + param + "=" + value;
        } else {
            self.queries = self.queries + "&" + param + "=" + value;
        }

        self
    }

    pub fn set(mut self, header: &str, value: &str) -> Self {
        let header = String::from(header) + ": " + value;
        self.header_list.append(&header).expect("out of memory");
        self
    }

    pub fn send_json(mut self, data: impl Serialize) -> Result<Response, Error> {
        let mut request = serde_json::to_vec(&data)
            .expect("unable to serialize data into JSON string")
            .into_iter();

        self.curl
            .read_function(move |dest| {
                let to_write = dest.len();
                let actual_written = request.len().min(to_write);

                request
                    .by_ref()
                    .take(actual_written)
                    .enumerate()
                    .for_each(|(i, byte)| dest[i] = byte);

                Ok(actual_written)
            })
            .unwrap(); // UNWRAP-SAFETY: This is always CURLE_OK.

        self.call()
    }

    pub fn call(mut self) -> Result<Response, Error> {
        let url = String::from(self.url) + &self.queries;
        self.curl.url(&url).expect("out of memory");

        // UNWRAP-SAFETY: HTTP is supported.
        self.curl.http_headers(self.header_list).unwrap();

        let mut response = Vec::with_capacity(8192);
        let mut transfer = self.curl.transfer();

        transfer
            .write_function(|src| {
                response.extend(src.iter().copied());
                Ok(src.len())
            })
            .unwrap(); // UNWRAP-SAFETY: This is always CURLE_OK.

        if let Err(err) = transfer.perform() {
            return Err(Error::Transport(err.description().into()));
        };

        drop(transfer);

        let response = Response {
            reader: Box::new(Cursor::new(response)),
        };

        // UNWRAP-SAFETY: The only error condition is when the curl version
        //                is too old. Let's just not support that.
        let response_code = self.curl.response_code().unwrap();
        if response_code >= 400 {
            return Err(Error::Status(response_code as u16, response));
        };

        Ok(response)
    }
}
