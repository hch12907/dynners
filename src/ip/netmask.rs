use std::fmt::Debug;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use thiserror::Error;

// #[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
// pub enum Network {
//     V4(NetworkV4),
//     V6(NetworkV6),
// }

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NetworkV4 {
    address: Ipv4Addr,
    mask: Ipv4Addr,
}

impl Default for NetworkV4 {
    fn default() -> Self {
        Self {
            address: Ipv4Addr::from(0),
            mask: Ipv4Addr::from(0),
        }
    }
}

impl Debug for NetworkV4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mask = u32::from(self.mask);

        if mask.leading_ones() + mask.trailing_zeros() == 32 {
            write!(f, "{}/{}", self.address, mask.leading_ones())
        } else {
            write!(f, "{}/{}", self.address, self.mask)
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct NetworkV6 {
    address: Ipv6Addr,
    mask: Ipv6Addr,
}

impl Default for NetworkV6 {
    fn default() -> Self {
        Self {
            address: Ipv6Addr::from(0),
            mask: Ipv6Addr::from(0),
        }
    }
}

impl Debug for NetworkV6 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mask = u128::from(self.mask);

        if mask.leading_ones() + mask.trailing_zeros() == 128 {
            write!(f, "{}/{}", self.address, mask.leading_ones())
        } else {
            write!(f, "{}/{}", self.address, self.mask)
        }
    }
}

fn v4_to_u32(ipv4: Ipv4Addr) -> u32 {
    u32::from_ne_bytes(ipv4.octets())
}

fn v6_to_u128(ipv6: Ipv6Addr) -> u128 {
    u128::from_ne_bytes(ipv6.octets())
}

// impl Network {
//     pub fn from_prefix(addr: IpAddr, prefix: u8) -> Self {
//         match addr {
//             IpAddr::V4(v4) => Network::V4(NetworkV4::from_prefix(v4, prefix)),
//             IpAddr::V6(v6) => Network::V6(NetworkV6::from_prefix(v6, prefix)),
//         }
//     }

//     pub fn from_mask(addr: IpAddr, mask: IpAddr) -> Self {
//         match (addr, mask) {
//             (IpAddr::V4(v4), IpAddr::V4(mask)) =>
//                 Network::V4(NetworkV4::from_mask(v4, mask)),
//             (IpAddr::V6(v6), IpAddr::V6(mask)) =>
//                 Network::V6(NetworkV6::from_mask(v6, mask)),

//             _ => panic!("nonsense mask creation")
//         }
//     }

//     pub fn in_range(&self, addr: IpAddr) -> bool {
//         match (self, addr) {
//             (Network::V4(v4), IpAddr::V4(addr)) => v4.in_range(addr),
//             (Network::V6(v6), IpAddr::V6(addr)) => v6.in_range(addr),

//             _ => panic!("nonsense range calculation")
//         }
//     }
// }

impl NetworkV4 {
    pub fn from_prefix(addr: Ipv4Addr, prefix: u8) -> Self {
        let bits = (32 - prefix) as u32;
        let mask = if bits < 32 { !0u32 >> bits << bits } else { 0 };

        Self {
            address: addr,
            mask: Ipv4Addr::from(mask),
        }
    }

    pub fn from_mask(addr: Ipv4Addr, mask: Ipv4Addr) -> Self {
        Self {
            address: addr,
            mask,
        }
    }

    pub fn in_range(&self, addr: Ipv4Addr) -> bool {
        (v4_to_u32(self.address) & v4_to_u32(self.mask)) == (v4_to_u32(addr) & v4_to_u32(self.mask))
    }
}

#[derive(Error, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum NetworkParseErr {
    #[error("a prefix or bitmask was not specified")]
    MaskUnspecified,

    #[error("an invalid address was specified")]
    InvalidAddress,

    #[error("an invalid netmask was provided")]
    InvalidMask,

    #[error("the provided netmask was too large for the protocol")]
    MaskTooLarge,
}

impl FromStr for NetworkV4 {
    type Err = NetworkParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some(slash) = s.find('/') else {
            return Err(NetworkParseErr::MaskUnspecified);
        };

        let (addr, mask) = s.split_at(slash);
        let mask = &mask[1..];

        let Ok(addr) = addr.parse::<Ipv4Addr>() else {
            return Err(NetworkParseErr::InvalidAddress);
        };

        if let Ok(prefix) = mask.parse::<u8>() {
            if prefix <= 32 {
                Ok(NetworkV4::from_prefix(addr, prefix))
            } else {
                Err(NetworkParseErr::MaskTooLarge)
            }
        } else if let Ok(mask) = mask.parse::<Ipv4Addr>() {
            Ok(NetworkV4::from_mask(addr, mask))
        } else {
            Err(NetworkParseErr::InvalidMask)
        }
    }
}

impl NetworkV6 {
    pub fn from_prefix(addr: Ipv6Addr, prefix: u8) -> Self {
        let bits = (128 - prefix) as u32;
        let mask = if bits < 128 {
            !0u128 >> bits << bits
        } else {
            0
        };
        Self {
            address: addr,
            mask: Ipv6Addr::from(mask),
        }
    }

    pub fn from_mask(addr: Ipv6Addr, mask: Ipv6Addr) -> Self {
        Self {
            address: addr,
            mask,
        }
    }

    pub fn in_range(&self, addr: Ipv6Addr) -> bool {
        (v6_to_u128(self.address) & v6_to_u128(self.mask))
            == (v6_to_u128(addr) & v6_to_u128(self.mask))
    }
}

impl FromStr for NetworkV6 {
    type Err = NetworkParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some(slash) = s.find('/') else {
            return Err(NetworkParseErr::MaskUnspecified);
        };

        let (addr, mask) = s.split_at(slash);
        let mask = &mask[1..];

        let Ok(addr) = addr.parse::<Ipv6Addr>() else {
            return Err(NetworkParseErr::InvalidAddress);
        };

        if let Ok(prefix) = mask.parse::<u8>() {
            if prefix <= 128 {
                Ok(NetworkV6::from_prefix(addr, prefix))
            } else {
                Err(NetworkParseErr::MaskTooLarge)
            }
        } else if let Ok(mask) = mask.parse::<Ipv6Addr>() {
            Ok(NetworkV6::from_mask(addr, mask))
        } else {
            Err(NetworkParseErr::InvalidMask)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::{NetworkV4, NetworkV6};

    #[test]
    fn network_v4() {
        let addr = "198.51.100.2".parse::<Ipv4Addr>().unwrap();
        let network = NetworkV4::from_prefix(addr, 24);

        assert!(network.in_range("198.51.100.0".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.51.100.1".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.51.100.98".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.51.100.255".parse::<Ipv4Addr>().unwrap()));
        assert!(!network.in_range("198.51.101.0".parse::<Ipv4Addr>().unwrap()));
        assert!(!network.in_range("198.52.101.132".parse::<Ipv4Addr>().unwrap()));

        let network = NetworkV4::from_prefix(addr, 8);
        assert!(network.in_range("198.0.0.0".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.10.0.1".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.51.100.0".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.51.100.1".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.128.100.0".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.255.100.1".parse::<Ipv4Addr>().unwrap()));
        assert!(!network.in_range("199.0.100.0".parse::<Ipv4Addr>().unwrap()));
        assert!(!network.in_range("255.255.255.0".parse::<Ipv4Addr>().unwrap()));

        let network = NetworkV4::from_prefix(addr, 0);
        assert!(network.in_range("198.0.0.0".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.10.0.1".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.51.100.0".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.51.100.1".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.128.100.0".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.255.100.1".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("199.0.100.0".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("255.255.255.0".parse::<Ipv4Addr>().unwrap()));

        let mask = "0.0.255.255".parse::<Ipv4Addr>().unwrap();
        let network = NetworkV4::from_mask(addr, mask);
        assert!(network.in_range("198.0.100.2".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.10.100.2".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("198.51.100.2".parse::<Ipv4Addr>().unwrap()));
        assert!(network.in_range("255.51.100.2".parse::<Ipv4Addr>().unwrap()));
        assert!(!network.in_range("198.0.98.0".parse::<Ipv4Addr>().unwrap()));
        assert!(!network.in_range("198.10.10.1".parse::<Ipv4Addr>().unwrap()));
        assert!(!network.in_range("198.51.100.0".parse::<Ipv4Addr>().unwrap()));
        assert!(!network.in_range("255.51.100.1".parse::<Ipv4Addr>().unwrap()));
    }

    #[test]
    fn network_v6() {
        let addr = "fe80::1234:cafe:babe:5678".parse::<Ipv6Addr>().unwrap();
        let network = NetworkV6::from_prefix(addr, 64);

        assert!(network.in_range("fe80::1".parse::<Ipv6Addr>().unwrap()));
        assert!(network.in_range("fe80::ff:1".parse::<Ipv6Addr>().unwrap()));
        assert!(network.in_range("fe80::8:7:6:5".parse::<Ipv6Addr>().unwrap()));
        assert!(!network.in_range("fe81::1".parse::<Ipv6Addr>().unwrap()));
        assert!(!network.in_range("2001:db8:ff::1".parse::<Ipv6Addr>().unwrap()));
        assert!(!network.in_range("8:7:6:5:4:3:2:1".parse::<Ipv6Addr>().unwrap()));

        let mask = "::ffff:ffff:ffff:ffff".parse::<Ipv6Addr>().unwrap();
        let network = NetworkV6::from_mask(addr, mask);

        assert!(network.in_range("2001:db8::1234:cafe:babe:5678".parse::<Ipv6Addr>().unwrap()));
        assert!(network.in_range("2001:db9::1234:cafe:babe:5678".parse::<Ipv6Addr>().unwrap()));
        assert!(network.in_range("fe80::1234:cafe:babe:5678".parse::<Ipv6Addr>().unwrap()));
        assert!(!network.in_range("fe80::2234:cafe:babe:5678".parse::<Ipv6Addr>().unwrap()));
        assert!(!network.in_range("2001::2234:cafe:babe:5678".parse::<Ipv6Addr>().unwrap()));
        assert!(!network.in_range("fe80:1234:cafe:babe::".parse::<Ipv6Addr>().unwrap()));

        let network = NetworkV6::from_prefix(addr, 0);
        assert!(network.in_range("2001:db8::1234:cafe:babe:5678".parse::<Ipv6Addr>().unwrap()));
        assert!(network.in_range("2001:db9::1234:cafe:babe:5678".parse::<Ipv6Addr>().unwrap()));
        assert!(network.in_range("fe80::1234:cafe:babe:5678".parse::<Ipv6Addr>().unwrap()));
        assert!(network.in_range("8:7:6:5:4:3:2:1".parse::<Ipv6Addr>().unwrap()));
        assert!(network.in_range("2001::2234:cafe:babe:5678".parse::<Ipv6Addr>().unwrap()));
        assert!(network.in_range("fe80:1234:cafe:babe::".parse::<Ipv6Addr>().unwrap()));
    }

    #[test]
    fn network_parse() {
        "192.168.0.1/255.255.255.255".parse::<NetworkV4>().unwrap();
        "192.168.0.1/255.255.0.255".parse::<NetworkV4>().unwrap();
        "192.168.0.1/0".parse::<NetworkV4>().unwrap();
        "192.168.0.1/16".parse::<NetworkV4>().unwrap();
        "192.168.0.1/32".parse::<NetworkV4>().unwrap();
        "255.255.255.255/255.255.255.255"
            .parse::<NetworkV4>()
            .unwrap();
        "255.255.255.255/255.255.0.255"
            .parse::<NetworkV4>()
            .unwrap();
        "255.255.255.255/32".parse::<NetworkV4>().unwrap();
        "255.255.255.255/0".parse::<NetworkV4>().unwrap();

        "fe80::/0".parse::<NetworkV6>().unwrap();
        "fe80::/10".parse::<NetworkV6>().unwrap();
        "fe80::/64".parse::<NetworkV6>().unwrap();
        "fe80::/128".parse::<NetworkV6>().unwrap();
        "::dead:beef/::ffff:ffff:ffff:ffff"
            .parse::<NetworkV6>()
            .unwrap();
        "::dead:beef/::f00f:ffff:f00f:ffff"
            .parse::<NetworkV6>()
            .unwrap();
        "2001:db8::/ffff:ffff::".parse::<NetworkV6>().unwrap();
        "2001:db8::/f0f0:fcfc::".parse::<NetworkV6>().unwrap();

        assert!("255.255.255.255/33".parse::<NetworkV4>().is_err());
        assert!("::/129".parse::<NetworkV6>().is_err())
    }
}
