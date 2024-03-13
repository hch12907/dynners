use std::net::{IpAddr, Ipv6Addr};

use data_encoding::BASE64;
use serde_derive::{Deserialize, Serialize};

use crate::http::{Error, Request};
use crate::util::FixedVec;

use super::{DdnsService, DdnsUpdateError};

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Config {
    username: Box<str>,
    password: Box<str>,
    gateway: Box<str>,
    protocol: Protocol,
    direction: Direction,
    dest_port: u16,
    allowed: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Tcp,
    Udp,
    Icmpv6,
}

#[derive(Deserialize, Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Outgoing,
    Incoming,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    config: Config,
    login_form: String,

    old_address: Option<Ipv6Addr>,
}

impl From<Config> for Service {
    fn from(config: Config) -> Self {
        let login_form = generate_form(&[
            ("username", &config.username),
            ("save", "Login"),
            ("submit-url", "/admin/login.asp"),
            ("encodePassword", &encode_password(&config.password)),
        ]);

        Self {
            config,
            login_form,
            old_address: None,
        }
    }
}

fn encode_password(password: &str) -> String {
    BASE64.encode(password.as_bytes())
}

fn generate_form(data: &[(&str, &str)]) -> String {
    let mut result = String::new();

    for (key, value) in data {
        result.push_str(key);
        result.push('=');

        for c in value.chars() {
            if c.is_ascii_alphanumeric() {
                result.push(c);
            } else if c == ' ' {
                result.push('+');
            } else if ['*', '-', '.', '_'].contains(&c) {
                result.push(c);
            } else if c.is_ascii_graphic() {
                result.push_str(&format!("%{:X}", c as u32 as u8));
            } else {
                result.extend(
                    c.to_string()
                        .as_bytes()
                        .iter()
                        .map(|byte| format!("%{:X}", byte)),
                );
            }
        }

        result.push('&');
    }

    use std::num::Wrapping;

    let mut chunks = result.as_bytes().iter().array_chunks::<4>();
    let mut checksum = Wrapping(0u32);
    for [d0, d1, d2, d3] in &mut chunks {
        checksum += Wrapping(u32::from(*d0) << 24);
        checksum += Wrapping(u32::from(*d1) << 16);
        checksum += Wrapping(u32::from(*d2) << 8);
        checksum += Wrapping(u32::from(*d3) << 0);
    }
    if let Some(mut iter) = chunks.into_remainder() {
        let mut i = 3;
        while let Some(d) = iter.next() {
            checksum += Wrapping(u32::from(*d) << (i << 3));
            i -= 1;
        }
    }

    let checksum = ((checksum.0 & 0xFFFF) + (checksum.0 >> 16)) & 0xFFFF;
    let checksum = (!checksum) & 0xFFFF;

    result.push_str(&format!("postSecurityFlag={}", checksum));

    result
}

impl Service {
    fn find_old_ip_index(&self) -> Result<Option<usize>, DdnsUpdateError> {
        let v6_filters =
            Request::get(&(self.config.gateway.to_string() + "/fw-ipportfilter-v6.asp"))
                .call()
                .map_err(|err| match err {
                    Error::Status(s, e) => {
                        let err = e.into_string().unwrap_or_else(|e| e.to_string());
                        DdnsUpdateError::Router1(s, err.into_boxed_str())
                    }
                    Error::Transport(e) => DdnsUpdateError::TransportError(e),
                })?;

        let content = v6_filters.into_string().map_err(|err| {
            let err = err.to_string();
            DdnsUpdateError::Router1(0, err.into_boxed_str())
        })?;

        let Some(table_start) =
            content.find("<form action=/boaform/formFilterV6 method=POST name=\"formFilterDel\">")
        else {
            return Ok(None);
        };

        let content = content.split_at(table_start).1;

        // The <tr> elements look like this:
        // <tr>
        // <td><input type="checkbox" name="select{N}" value="ON"></td>
        // <td>Incoming</td>
        // <td>TCP</td>
        // <td></td>
        // <td></td>
        // <td>2001:db8::1/0</td>
        // <td>12298</td>
        // <td>Always</td>
        // <td>Allow</td>
        // <td><a href="#" onclick="editFilterClick(...)"><img ...></td>
        // </tr>
        let parse_tr = |tr: &str| -> Result<Option<usize>, ()> {
            let tr = tr.split("</tr>").next().ok_or(())?;

            let mut tds = tr.split("<td>");
            let _ = tds.next(); // skip the initial <tr> tag

            let name = tds
                .next()
                .ok_or(())?
                .split("<input type=\"checkbox\" name=\"")
                .nth(1)
                .ok_or(())?
                .split_once('"')
                .ok_or(())?
                .0;

            let protocol = tds.nth(1).ok_or(())?.split_once("</td>").ok_or(())?.0;

            let mut dest_ip = tds.nth(2).ok_or(())?.split_once("</td>").ok_or(())?.0;

            // remove the CIDR prefix from dest IP
            if dest_ip.contains('-') {
                // this daemon never creates ranged dest IPs, just skip this entry
                // if we see one
                return Ok(None);
            }
            if dest_ip.contains('/') {
                dest_ip = dest_ip.split('/').next().ok_or(())?;
            }

            let dest_port = tds.next().ok_or(())?.split_once("</td>").ok_or(())?.0;

            if dest_port.contains("-") {
                // same logic as ranged dest IP
                return Ok(None);
            }

            let dest_ip = if !dest_ip.is_empty() {
                dest_ip.parse::<Ipv6Addr>().map_err(|_| ())?
            } else {
                return Ok(None);
            };

            let same_protocol = match protocol.to_ascii_lowercase().as_ref() {
                "tcp" => self.config.protocol == Protocol::Tcp,
                "udp" => self.config.protocol == Protocol::Udp,
                "icmpv6" => self.config.protocol == Protocol::Icmpv6,
                _ => Err(())?,
            };

            if same_protocol
                && Some(dest_ip) == self.old_address
                && dest_port.parse::<u16>().map_err(|_| ())? == self.config.dest_port
            {
                Ok(Some(
                    name.strip_prefix("select")
                        .ok_or(())?
                        .parse::<usize>()
                        .map_err(|_| ())?,
                ))
            } else {
                Ok(None)
            }
        };

        let mut remaining_content = content;

        loop {
            if let Some(i) = remaining_content.find("<tr><td>") {
                remaining_content = &remaining_content[i..];
                match parse_tr(remaining_content) {
                    Ok(Some(idx)) => return Ok(Some(idx)),

                    Ok(None) => {
                        // This tr is not what we wanted, continue
                        remaining_content = &remaining_content[8..];
                        continue;
                    }

                    Err(()) => {
                        return Err(DdnsUpdateError::Router1(
                            0,
                            "Unable to parse v6 filter returned by router".into(),
                        ))
                    }
                }
            } else {
                break;
            }
        }

        Ok(None)
    }
}

impl DdnsService for Service {
    fn update_record(&mut self, ips: &[IpAddr]) -> Result<FixedVec<IpAddr, 2>, DdnsUpdateError> {
        let ipv6 = ips.iter().find(|ip| ip.is_ipv6());

        let login_request =
            Request::post(&(self.config.gateway.to_string() + "/boaform/admin/formLogin"))
                .set("Referer", self.config.gateway.as_ref())
                .set("Content-Type", "application/x-www-form-urlencoded")
                .send_form(&self.login_form);

        let mut result: FixedVec<IpAddr, 2> = FixedVec::new();

        match login_request {
            Ok(req) => (),
            Err(Error::Status(s, e)) => {
                let err = e.into_string().unwrap_or_else(|e| e.to_string());
                Err(DdnsUpdateError::Router1(s, err.into_boxed_str()))?
            }
            Err(Error::Transport(e)) => Err(DdnsUpdateError::TransportError(e))?,
        }

        if let Some(ipv6) = ipv6 {
            let old_ip_index = self.find_old_ip_index();

            if let Some(idx) = old_ip_index? {
                let select = format!("select{}", idx);

                Request::post(&(self.config.gateway.to_string() + "/boaform/formFilterV6"))
                    .set("Origin", self.config.gateway.as_ref())
                    .set(
                        "Referer",
                        &(self.config.gateway.to_string() + "/fw-ipportfilter-v6.asp"),
                    )
                    .set("Content-Type", "application/x-www-form-urlencoded")
                    .send_form(&generate_form(&[
                        (&select, "ON"),
                        ("deleteSelFilterIpPort", "Delete Selected"),
                        ("submit-url", "/fw-ipportfilter-v6.asp"),
                    ]))
                    .map_err(|err| match err {
                        Error::Status(s, e) => {
                            let err = e.into_string().unwrap_or_else(|e| e.to_string());
                            DdnsUpdateError::Router1(s, err.into_boxed_str())
                        }
                        Error::Transport(e) => DdnsUpdateError::TransportError(e),
                    })?;
            }

            let direction = match self.config.direction {
                Direction::Outgoing => "0",
                Direction::Incoming => "1",
            };

            let protocol = match self.config.protocol {
                Protocol::Tcp => "1",
                Protocol::Udp => "2",
                Protocol::Icmpv6 => "3",
            };

            let filter_mode = if self.config.allowed { "Allow" } else { "Deny" };

            let firewall_request =
                Request::post(&(self.config.gateway.to_string() + "/boaform/formFilterV6"))
                    .set("Origin", self.config.gateway.as_ref())
                    .set(
                        "Referer",
                        &(self.config.gateway.to_string() + "/fw-ipportfilter-v6.asp"),
                    )
                    .set("Content-Type", "application/x-www-form-urlencoded")
                    .send_form(&generate_form(&[
                        // Direction. 0 = Outgoing; 1 = Incoming
                        ("dir", direction),
                        // Transport protocol. 1 = TCP; 2 = UDP; 3 = ICMP
                        ("protocol", protocol),
                        // Filter mode. Deny or Allow.
                        ("filterMode", filter_mode),
                        ("sip6Start", ""),
                        ("sip6End", ""),
                        ("sip6PrefixLen", ""),
                        ("dip6Start", &ipv6.to_string()),
                        ("dip6End", ""),
                        // NOTE: the prefix lengths don't seem to work?! Needs more testing
                        ("dip6PrefixLen", ""),
                        ("sfromPort", ""),
                        ("stoPort", ""),
                        ("dfromPort", &self.config.dest_port.to_string()),
                        ("dtoPort", ""),
                        // Seems to be hardcoded to 65536.
                        ("wanif", "65536"),
                        ("schedList", ""),
                        ("addFilterIpPort", "Add Rule"),
                        ("select_id", ""),
                        ("submit-url", "/fw-ipportfilter-v6.asp"),
                    ]));

            firewall_request.map_err(|err| match err {
                Error::Status(s, e) => {
                    let err = e.into_string().unwrap_or_else(|e| e.to_string());
                    DdnsUpdateError::Router1(s, err.into_boxed_str())
                }
                Error::Transport(e) => DdnsUpdateError::TransportError(e),
            })?;

            result.push(*ipv6);
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum() {
        let gen = generate_form(&[
            ("username", "admin"),
            ("save", "Login"),
            ("submit-url", "/admin/login.asp"),
            ("encodePassword", &encode_password("admin")),
        ]);
        assert_eq!(
            &gen,
            "username=admin&save=Login&submit-url=%2Fadmin%2Flogin.asp&encodePassword=YWRtaW4%3D&postSecurityFlag=47346"
        );
    }
}
