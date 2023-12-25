use std::collections::HashMap;
use std::io::{self, Read, Write, Bytes};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::num::Wrapping;
use std::time::{UNIX_EPOCH, SystemTime};

/// The current persistent state file version. The program must reject state
/// files newer than this, and must upgrade or reject state files older than
/// this.
const STATE_VERSION: u32 = 1;

/// This struct stores all program states that will survive between multiple
/// sessions. This is to prevent dynners from sending excessive update requests
/// to the DDNS providers in scenarios like user restarting the program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistentState {
    /// The magic number present in every persistent state file.
    /// It must be "dynners\0".
    //pub magic: [u8; 8],

    /// The version of the persistent state. It is not required to be in sync
    /// with the version of dynners itself, increment only when the file format
    /// changes.
    pub version: u32,

    /// Unix timestamp in seconds, it is stored for debugging? purposes and has
    /// no practical meaning beyond that.
    pub update_timestamp: u64,

    /// The config file hash. If the config file is modified, the persistent
    /// state will be invalidated.
    pub config_hash: u64,

    /// The IP addresses from last session. On disk, each entry will be stored
    /// as a tuple of:
    ///     - name_length: u32
    ///     - name: string,
    ///     - ip_type: u8 (represented using the enum IpType)
    ///     - ip: (u32 | u128) with size depending on ip_type
    pub ip_addresses: HashMap<Box<str>, IpAddr>,
}

enum IpType {
    Ipv4 = 0,
    Ipv6 = 1, 
}

fn hash_bytes(s: &[u8]) -> u64 {
    // Absolutely zero thinking went into the designing of this algorithm.
    // Don't take it too seriously. This can be changed as needed.
    let hash1 = crc32fast::hash(s);

    let mut hash2 = Wrapping(hash1);
    for byte in s {
        hash2 *= 65539;
        hash2 += *byte as u32;
    }

    ((hash1 as u64) << 32) | (hash2.0 as u64)
}

impl PersistentState {
    pub fn new(config: &str) -> Self {
        Self::new_with_config_hash(hash_bytes(config.as_bytes()))
    }

    pub fn new_with_config_hash(config_hash: u64) -> Self {
        let current_timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(duration) => duration.as_secs(),
            Err(_) => 0,
        };

        Self {
            version: STATE_VERSION,
            update_timestamp: current_timestamp,
            config_hash,
            ip_addresses: HashMap::new(),
        }
    }

    pub fn is_same_config_file(&self, config: &str) -> bool {
        self.config_hash == hash_bytes(config.as_bytes())
    }

    // If the configuration file is found to have changed, invalidate this
    // persistent state and return false.
    pub fn validate_against(&mut self, config: &str) -> bool {
        if !self.is_same_config_file(config) {
            self.ip_addresses.clear();
            self.config_hash = hash_bytes(config.as_bytes());
            self.update_timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
                Ok(duration) => duration.as_secs(),
                Err(_) => 0,
            };

            false
        } else {
            true
        }
    }

    pub fn from_reader<R: Read>(reader: R) -> io::Result<Self> {
        let mut iter = reader.bytes();

        let read_field = |iter: &mut Bytes<R>, name, len| {
            let read = iter
                .by_ref()
                .take(len)
                .collect::<io::Result<Box<[u8]>>>()?;

            if read.len() == len {
                Ok(read)
            } else {
                let message = String::from("EOF while reading persistent state file for ") + name;
                Err(io::Error::new(io::ErrorKind::UnexpectedEof, message))
            }
        };

        let magic = read_field(&mut iter, "magic", 8)?;
        if *magic != *b"dynners\0" {
            let message = "unexpected file format: invalid magic";
            Err(io::Error::new(io::ErrorKind::InvalidInput, message))?   
        }

        let version = read_field(&mut iter, "version", 4)?;
        // UNWRAP-SAFETY: length is confirmed to be 4 bytes by read_field()
        // This will be a common theme in this function
        let version = <[u8; 4]>::try_from(&*version).unwrap();

        // Reject newer persistence state files.
        if u32::from_le_bytes(version) > STATE_VERSION {
            let message = "the persistent state file is too new";
            Err(io::Error::new(io::ErrorKind::Unsupported, message))?   
        }

        let update_timestamp = read_field(&mut iter, "update timestamp", 8)?;
        let update_timestamp = <[u8; 8]>::try_from(&*update_timestamp).unwrap();

        let config_hash = read_field(&mut iter, "config hash", 8)?;
        let config_hash = <[u8; 8]>::try_from(&*config_hash).unwrap();

        let mut ip_addresses = HashMap::new();
        while let Ok(name_len) = read_field(&mut iter, "name length", 4) {
            let name_len = <[u8; 4]>::try_from(&*name_len).unwrap();
            let name_len = u32::from_le_bytes(name_len);

            if name_len == 0 {
                break
            }

            let Ok(name) = String::from_utf8(
                Vec::from(read_field(&mut iter, "version", name_len as usize)?)
            ) else {
                let message = "unexpected non-UTF8 IP address name";
                Err(io::Error::new(io::ErrorKind::InvalidInput, message))?
            };

            let ip_type = read_field(&mut iter, "IP type", 1)?[0];

            let ip = if ip_type == IpType::Ipv4 as u8 {
                let ip_raw = read_field(&mut iter, "IPv4 address", 4)?;
                let ip = <[u8; 4]>::try_from(&*ip_raw).unwrap();
                IpAddr::V4(Ipv4Addr::from(u32::from_le_bytes(ip)))
            } else if ip_type == IpType::Ipv6 as u8 {
                let ip_raw = read_field(&mut iter, "IPv6 address", 16)?;
                let ip = <[u8; 16]>::try_from(&*ip_raw).unwrap();
                IpAddr::V6(Ipv6Addr::from(u128::from_le_bytes(ip)))
            } else {
                let message = "unexpected IP type";
                Err(io::Error::new(io::ErrorKind::InvalidInput, message))?
            };

            ip_addresses.insert(name.into_boxed_str(), ip);
        }
        
        Ok(Self {
            version: u32::from_le_bytes(version),
            update_timestamp: u64::from_le_bytes(update_timestamp),
            config_hash: u64::from_le_bytes(config_hash),
            ip_addresses,
        })
    }

    pub fn write_to<W: Write>(&self, writer: W) -> io::Result<()> {
        let mut writer = writer;

        writer.write(b"dynners\0")?;
        writer.write(&self.version.to_le_bytes())?;
        writer.write(&self.update_timestamp.to_le_bytes())?;
        writer.write(&self.config_hash.to_le_bytes())?;

        for (name, ip) in &self.ip_addresses {
            writer.write(&(name.len() as u32).to_le_bytes())?;
            writer.write(name.as_bytes())?;

            match ip {
                IpAddr::V4(v4) => {
                    writer.write(&[IpType::Ipv4 as u8])?;
                    writer.write(&u32::from(*v4).to_le_bytes())?;
                }

                IpAddr::V6(v6) => {
                    writer.write(&[IpType::Ipv6 as u8])?;
                    writer.write(&u128::from(*v6).to_le_bytes())?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn reversible() {
        // Preparation of state
        let mut state = PersistentState::new(
            "hello world, please hash me uwu"
        );
        state.ip_addresses.insert(
            "hello".into(),
            Ipv4Addr::new(192, 168, 100, 200).into()
        );
        state.ip_addresses.insert(
            "你好".into(),
            Ipv4Addr::new(172, 19, 10, 20).into()
        );
        state.ip_addresses.insert(
            "world".into(),
            Ipv6Addr::new(0x2001, 0xdb8, 0x1234, 0x4567, 0xcafe, 0xbabe, 0xdead, 0xbeef).into()
        );
        state.ip_addresses.insert(
            "世界".into(),
            Ipv6Addr::new(0x2001, 0xdb8, 0x1111, 0x2222, 0x1337, 0x0ff1, 0xce00, 0x4b1d).into()
        );

        // Actual test begins here
        let mut buffer = Cursor::new(vec![]);
        assert!(state.write_to(&mut buffer).is_ok());
        assert!(buffer.position() > 0);

        println!("{:?}", &buffer);

        buffer.set_position(0);
        let state_read = PersistentState::from_reader(buffer).unwrap();
        
        assert_eq!(state.version, state_read.version);
        assert_eq!(state.update_timestamp, state_read.update_timestamp);
        assert_eq!(state.config_hash, state_read.config_hash);
        assert_eq!(state.ip_addresses, state_read.ip_addresses);
    }

    #[test]
    fn error_extravaganza() {
        // Invalid magic number
        let buffer = Cursor::new(vec![100, 121, 110, 111, 101, 114, 115, 0]);
        assert!(PersistentState::from_reader(buffer).is_err());
    }
}
