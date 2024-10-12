use std::net::IpAddr;

use serde_derive::{Deserialize, Serialize};

use crate::http::{Error, Request, Response};
use crate::util::FixedVec;

use super::{one_or_more_string, DdnsService, DdnsUpdateError};

type ZoneId = u128;
type RecordId = u128;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    ttl: u32,

    proxied: bool,

    token: Box<str>,

    #[serde(deserialize_with = "one_or_more_string")]
    domains: Vec<Box<str>>,
}

pub struct Service {
    config: Config,
    cached_records: Vec<Record>,
}

struct Record {
    zone_id: ZoneId,
    id: RecordId,
    domain: Box<str>,
    kind: RecordKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RecordKind {
    A,
    Aaaa,
}

impl From<Config> for Service {
    fn from(config: Config) -> Self {
        let mut config = config;
        config.token = (String::from("Bearer ") + &config.token).into();
        Self {
            config,
            cached_records: Vec::new(),
        }
    }
}

impl Service {
    fn parse_error(&self, response: Response) -> Result<(u32, Box<str>), String> {
        let resp_json = response
            .into_json::<serde_json::Value>()
            .map_err(|e| String::from("unable to parse response as JSON:") + &e.to_string())?;

        let errors = resp_json
            .get("errors")
            .ok_or_else(|| String::from("expected map"))?;

        let error = errors
            .get(0)
            .ok_or_else(|| String::from("expected array"))?;

        let code = error
            .get("code")
            .and_then(|c| c.as_u64())
            .ok_or_else(|| String::from("expected number"))?;

        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .ok_or_else(|| String::from("expected string"))?
            .to_owned()
            .into_boxed_str();

        Ok((code as u32, message))
    }

    fn parse_and_check_response(
        &self,
        response: Result<Response, Error>,
    ) -> Result<serde_json::Value, DdnsUpdateError> {
        let response = match response {
            Ok(r) => r
                .into_json::<serde_json::Value>()
                .map_err(|e| DdnsUpdateError::Json(e.to_string().into()))?,
            Err(Error::Status(_, resp)) => {
                let (code, message) = self.parse_error(resp).map_err(|ref e| {
                    let error = String::from("unexpected error message structure - ");
                    DdnsUpdateError::Json((error + e).into_boxed_str())
                })?;
                Err(DdnsUpdateError::Cloudflare(code, message))?
            }
            Err(Error::Transport(tp)) => {
                Err(DdnsUpdateError::TransportError(tp.to_string().into()))?
            }
        };

        // A sanity check.
        let success = response
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !success {
            return Err(DdnsUpdateError::Json(
                "cloudflare returned success=false?".into(),
            ));
        };

        Ok(response)
    }

    fn get_zones(&self) -> Result<Vec<ZoneId>, DdnsUpdateError> {
        let response = Request::get("https://api.cloudflare.com/client/v4/zones/")
            .set("Content-Type", "application/json")
            .set("Authorization", &self.config.token)
            .call();

        let response = self.parse_and_check_response(response)?;

        let results = response.get("result").and_then(|v| v.as_array());
        let Some(zones) = results else {
            return Err(DdnsUpdateError::Json("cloudflare returned 0 zones".into()));
        };

        let mut zone_ids = Vec::with_capacity(zones.len());

        for zone in zones {
            let Some(id) = zone.get("id").and_then(|v| v.as_str()) else {
                return Err(DdnsUpdateError::Json("zone has no id?".into()));
            };

            let Some(permissions) = zone.get("permissions").and_then(|v| v.as_array()) else {
                continue;
            };

            let permissions = permissions
                .iter()
                .map(|perm| perm.as_str().unwrap_or_default())
                .filter(|perm| perm.starts_with("#dns_records"));

            let mut can_read = false;
            let mut can_edit = false;

            for perm in permissions {
                if perm.contains("read") {
                    can_read = true;
                }
                if perm.contains("edit") {
                    can_edit = true;
                }
            }

            if can_read && can_edit {
                let Ok(id) = ZoneId::from_str_radix(id, 16) else {
                    Err(DdnsUpdateError::Json("id is not a u128 number".into()))?
                };
                zone_ids.push(id);
            }
        }

        Ok(zone_ids)
    }

    fn get_records(&self, zone_id: ZoneId) -> Result<Vec<Record>, DdnsUpdateError> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{:x}/dns_records",
            zone_id
        );

        let response = Request::get(&url)
            .set("Content-Type", "application/json")
            .set("Authorization", &self.config.token)
            .call();

        let response = self.parse_and_check_response(response)?;

        let results = response.get("result").and_then(|v| v.as_array());
        let Some(records) = results else {
            return Err(DdnsUpdateError::Json(
                "cloudflare returned 0 records".into(),
            ));
        };

        let mut returned_records = Vec::new();
        for record in records {
            let Some(id) = record.get("id").and_then(|v| v.as_str()) else {
                return Err(DdnsUpdateError::Json("record has no id?".into()));
            };

            let Ok(id) = RecordId::from_str_radix(id, 16) else {
                Err(DdnsUpdateError::Json("id is not a u128 number".into()))?
            };

            let Some(domain) = record.get("name").and_then(|v| v.as_str()) else {
                return Err(DdnsUpdateError::Json("record has no name?".into()));
            };

            let Some(ty) = record.get("type").and_then(|v| v.as_str()) else {
                return Err(DdnsUpdateError::Json("record has no type?".into()));
            };

            let kind = match ty {
                "A" => RecordKind::A,
                "AAAA" => RecordKind::Aaaa,
                _ => continue,
            };

            returned_records.push(Record {
                zone_id,
                id,
                domain: domain.into(),
                kind,
            });
        }

        Ok(returned_records)
    }

    fn put_record(&self, record: &Record, ip: IpAddr) -> Result<(), DdnsUpdateError> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/zones/{:x}/dns_records/{:x}",
            record.zone_id, record.id
        );

        let response = Request::put(&url)
            .set("Authorization", &self.config.token)
            .send_json(serde_json::json!({
                "content": ip.to_string(),
                "name": record.domain.as_ref(),
                "proxied": self.config.proxied,
                "type": if ip.is_ipv4() { "A" } else { "AAAA" },
                "ttl": self.config.ttl,
            }));

        self.parse_and_check_response(response)?;

        Ok(())
    }
}

impl DdnsService for Service {
    fn update_record(&mut self, ips: &[IpAddr]) -> Result<FixedVec<IpAddr, 2>, DdnsUpdateError> {
        if self.cached_records.is_empty() {
            for zone in self.get_zones()? {
                for record in self.get_records(zone)? {
                    if self.config.domains.iter().any(|d| *d == record.domain) {
                        self.cached_records.push(record)
                    }
                }
            }
        }

        let ipv4 = ips.iter().find(|ip| ip.is_ipv4());
        let ipv6 = ips.iter().find(|ip| ip.is_ipv6());

        for record in &self.cached_records {
            if record.kind == RecordKind::A && ipv4.is_some() {
                self.put_record(record, *ipv4.unwrap())?;
            } else if record.kind == RecordKind::Aaaa && ipv6.is_some() {
                self.put_record(record, *ipv6.unwrap())?;
            }
        }

        let mut result = FixedVec::new();
        if let Some(ipv4) = ipv4 {
            result.push(*ipv4);
        }
        if let Some(ipv6) = ipv6 {
            result.push(*ipv6);
        }

        Ok(result)
    }
}
