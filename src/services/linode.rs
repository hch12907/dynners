use std::net::IpAddr;

use serde_derive::{Deserialize, Serialize};

use crate::http::{Error, Request, Response};
use crate::util::FixedVec;

use super::{one_or_more_string, DdnsService, DdnsUpdateError};

type RecordId = u64;
type DomainId = u64;

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    token: Box<str>,

    #[serde(deserialize_with = "one_or_more_string")]
    domains: Vec<Box<str>>,

    /// The time to live expressed in seconds.
    ///
    /// Values that are not multiples of 300 will be rounded to the nearest
    /// multiple by the Linode API.
    /// See: https://www.linode.com/docs/api/domains/#domain-record-update__request-body-schema
    ttl: u32,
}

pub struct Service {
    config: Config,
    cached_records: Vec<Record>,
}

#[derive(Debug, Clone)]
struct Domain {
    id: DomainId,

    name: Box<str>,
}

#[derive(Debug)]
struct Record {
    /// Linode uses a master domain (example.com) and encodes different
    /// records inside it.

    /// The ID of the record, e.g. ID of sub.example.com
    id: RecordId,

    /// The domain associated to the record.
    domain_id: DomainId,

    /// The actual name of the record.
    name: Box<str>,

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
    fn parse_error(&self, response: Response) -> Result<(Box<str>, Box<str>), String> {
        let resp_json = response
            .into_json::<serde_json::Value>()
            .map_err(|e| String::from("unable to parse response as JSON:") + &e.to_string())?;

        let errors = resp_json
            .get("errors")
            .ok_or_else(|| String::from("expected map"))?;

        let error = errors
            .get(0)
            .ok_or_else(|| String::from("expected array"))?;

        // When Linode returns an error it may signal to us if a field
        // in the request is malformed, in which case the key `field` is
        // populated in this response.
        //
        // If no key `field` exists, we revert to an empty string.

        let field = error
            .get("field")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_owned()
            .into_boxed_str();

        let reason = error
            .get("reason")
            .and_then(|m| m.as_str())
            .ok_or_else(|| String::from("expected string"))?
            .to_owned()
            .into_boxed_str();

        Ok((field, reason))
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
                let (field, reason) = self.parse_error(resp).map_err(|ref e| {
                    let error = String::from("unexpected error message structure - ");
                    DdnsUpdateError::Json((error + e).into_boxed_str())
                })?;

                let error_message: Box<str> = if field.is_empty() {
                    reason
                } else {
                    format!("{} (field = {})", reason, field).into()
                };

                Err(DdnsUpdateError::Linode(error_message))?
            }
            Err(Error::Transport(tp)) => {
                Err(DdnsUpdateError::TransportError(tp.to_string().into()))?
            }
        };

        Ok(response)
    }

    /// See:
    ///   - https://www.linode.com/docs/api/domains/#domains-list
    ///   - https://www.linode.com/docs/api/domains/#domains-list__responses
    fn get_domains(&self) -> Result<Vec<Domain>, DdnsUpdateError> {
        let response = Request::get("https://api.linode.com/v4/domains")
            .set("Content-Type", "application/json")
            .set("Authorization", &self.config.token)
            .call();

        let response = self.parse_and_check_response(response)?;

        let results = response.get("data").and_then(|v| v.as_array());
        let Some(domains) = results else {
            return Err(DdnsUpdateError::Json("linode returned 0 domains".into()));
        };

        let mut domains_ret = Vec::with_capacity(domains.len());

        for domain in domains {
            let Some(id) = domain.get("id").and_then(|v| v.as_number()) else {
                return Err(DdnsUpdateError::Json("domain has no id?".into()));
            };

            let Some(id) = id.as_u64() else {
                Err(DdnsUpdateError::Json(
                    "cannot convert domain ID to u64".into(),
                ))?
            };

            let Some(name) = domain.get("domain").and_then(|v| v.as_str()) else {
                return Err(DdnsUpdateError::Json("domain has no domain name?".into()));
            };

            domains_ret.push(Domain {
                id: DomainId::from(id),
                name: name.into(),
            });
        }

        Ok(domains_ret)
    }

    /// See:
    ///   - https://www.linode.com/docs/api/domains/#domain-records-list
    ///   - https://www.linode.com/docs/api/domains/#domain-records-list__responses
    fn get_records(&self, domain: Domain) -> Result<Vec<Record>, DdnsUpdateError> {
        let url = format!("https://api.linode.com/v4/domains/{}/records", domain.id);

        let response = Request::get(&url)
            .set("Content-Type", "application/json")
            .set("Authorization", &self.config.token)
            .call();

        let response = self.parse_and_check_response(response)?;

        let results = response.get("data").and_then(|v| v.as_array());
        let Some(records) = results else {
            return Err(DdnsUpdateError::Json("linode returned 0 records".into()));
        };

        let mut returned_records = Vec::new();
        for record in records {
            let Some(id) = record.get("id").and_then(|v| v.as_number()) else {
                return Err(DdnsUpdateError::Json("record has no id?".into()));
            };

            let Some(id) = id.as_u64() else {
                return Err(DdnsUpdateError::Json("id is not a u64 number".into()));
            };

            let Some(name) = record.get("name").and_then(|v| v.as_str()) else {
                return Err(DdnsUpdateError::Json("record has no name?".into()));
            };

            // The `name` field contains only the subdomain.
            // For example, test.example.com will have its `name` set to "test".
            // So we concatenate it to obtain the FQDN.
            let fqdn: Box<str> = if name.is_empty() {
                domain.name.clone()
            } else {
                format!("{}.{}", name, domain.name).into()
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
                id,
                domain_id: domain.id,
                name: fqdn,
                kind,
            });
        }

        Ok(returned_records)
    }

    /// See: https://www.linode.com/docs/api/domains/#domain-record-update__request-body-schema
    fn put_record(&self, record: &Record, ip: IpAddr) -> Result<(), DdnsUpdateError> {
        let url = format!(
            "https://api.linode.com/v4/domains/{}/records/{}",
            record.domain_id, record.id
        );

        // We don't have to include the name again, just the target and TTL.

        let response = Request::put(&url)
            .set("Authorization", &self.config.token)
            .send_json(serde_json::json!({
                "target": ip.to_string(),
                "ttl_sec": self.config.ttl,
            }));

        self.parse_and_check_response(response)?;

        Ok(())
    }
}

impl DdnsService for Service {
    fn update_record(&mut self, ips: &[IpAddr]) -> Result<FixedVec<IpAddr, 2>, DdnsUpdateError> {
        if self.cached_records.is_empty() {
            for domain in self.get_domains()? {
                for record in self.get_records(domain)? {
                    if self.config.domains.iter().any(|d| *d == record.name) {
                        self.cached_records.push(record)
                    }
                }
            }
        }

        let ipv4 = ips.iter().find(|ip| ip.is_ipv4());
        let ipv6 = ips.iter().find(|ip| ip.is_ipv6());

        for record in &self.cached_records {
            if ipv4.is_some() && record.kind == RecordKind::A {
                self.put_record(record, *ipv4.unwrap())?;
            } else if ipv6.is_some() && record.kind == RecordKind::Aaaa {
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
