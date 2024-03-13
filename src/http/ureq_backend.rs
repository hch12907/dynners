use serde::Serialize;
use ureq;

use crate::GENERAL_CONFIG;

use super::{Error, Response};

pub struct Request {
    inner: ureq::Request,
}

impl Request {
    pub fn get(url: &str) -> Self {
        let inner = ureq::get(url).set("User-Agent", &GENERAL_CONFIG.get().unwrap().user_agent);
        Self { inner }
    }

    pub fn post(url: &str) -> Self {
        let inner = ureq::post(url).set("User-Agent", &GENERAL_CONFIG.get().unwrap().user_agent);
        Self { inner }
    }

    pub fn put(url: &str) -> Self {
        let inner = ureq::put(url).set("User-Agent", &GENERAL_CONFIG.get().unwrap().user_agent);
        Self { inner }
    }

    pub fn query(mut self, param: &str, value: &str) -> Self {
        self.inner = self.inner.query(param, value);
        self
    }

    pub fn set(mut self, header: &str, value: &str) -> Self {
        self.inner = self.inner.set(header, value);
        self
    }

    pub fn send_form(self, data: &str) -> Result<Response, Error> {
        self.inner
            .send_bytes(data.as_bytes())
            .map_err(|e| match e {
                ureq::Error::Status(code, resp) => Error::Status(
                    code,
                    Response {
                        reader: resp.into_reader(),
                    },
                ),
                ureq::Error::Transport(tp) => Error::Transport(tp.to_string().into()),
            })
            .map(|resp| Response {
                reader: resp.into_reader(),
            })
    }

    pub fn send_json(self, data: impl Serialize) -> Result<Response, Error> {
        self.inner
            .send_json(data)
            .map_err(|e| match e {
                ureq::Error::Status(code, resp) => Error::Status(
                    code,
                    Response {
                        reader: resp.into_reader(),
                    },
                ),
                ureq::Error::Transport(tp) => Error::Transport(tp.to_string().into()),
            })
            .map(|resp| Response {
                reader: resp.into_reader(),
            })
    }

    pub fn call(self) -> Result<Response, Error> {
        self.inner
            .call()
            .map_err(|e| match e {
                ureq::Error::Status(code, resp) => Error::Status(
                    code,
                    Response {
                        reader: resp.into_reader(),
                    },
                ),
                ureq::Error::Transport(tp) => Error::Transport(tp.to_string().into()),
            })
            .map(|resp| Response {
                reader: resp.into_reader(),
            })
    }
}
