use std::net::{Ipv4Addr, Ipv6Addr};

use super::netmask::{NetworkV4, NetworkV6};

pub(super) fn get_interface_v4_addresses(iface: &str, mask: &NetworkV4) -> Option<Ipv4Addr> {
    os::get_interface_v4_addresses(iface, mask)
}

pub(super) fn get_interface_v6_addresses(iface: &str, mask: &NetworkV6) -> Option<Ipv6Addr> {
    os::get_interface_v6_addresses(iface, mask)
}

#[cfg(target_family = "unix")]
mod os {
    use std::ffi::CStr;
    use std::mem::MaybeUninit;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    use libc;

    use crate::ip::netmask::{NetworkV4, NetworkV6};

    fn transverse_ifaddr(iface: &str) -> Vec<IpAddr> {
        let mut ip_addrs = Vec::new();

        // SAFETY: if getifaddrs() succeeds, ifaddrs is guaranteed to be
        // initialized. The lifetime is undetermined (hence 'static) until we
        // free it later.
        let ifaddrs = unsafe {
            let mut ifaddrs = MaybeUninit::<&'static mut libc::ifaddrs>::uninit();

            if libc::getifaddrs(&mut ifaddrs as *mut _ as _) < 0 {
                return ip_addrs;
            }

            ifaddrs.assume_init()
        };

        let mut current = ifaddrs as *const libc::ifaddrs;

        while !current.is_null() {
            // SAFETY: Nullness is already checked above.
            let ifaddr = unsafe { &*current };

            // SAFETY: the name returned by the OS is a safe, null-terminated
            // string. At least I hope it is so.
            let ifa_name = unsafe { CStr::from_ptr(ifaddr.ifa_name) };

            if ifa_name.to_string_lossy() != iface {
                current = ifaddr.ifa_next as *const _;
                continue;
            }

            if !ifaddr.ifa_addr.is_null() {
                // SAFETY: nullness is checked above.
                let ifa_addr = unsafe { *ifaddr.ifa_addr };

                if ifa_addr.sa_family == libc::AF_INET as u16 {
                    // SAFETY: the type of the pointer is given by sa_family
                    let ifa_addr = unsafe { *(ifaddr.ifa_addr as *mut libc::sockaddr_in) };
                    let raw = u32::from_be(ifa_addr.sin_addr.s_addr);
                    let ipv4 = Ipv4Addr::from(raw);
                    ip_addrs.push(IpAddr::V4(ipv4))
                } else if ifa_addr.sa_family == libc::AF_INET6 as u16 {
                    // SAFETY: the type of the pointer is given by sa_family
                    let ifa_addr = unsafe { *(ifaddr.ifa_addr as *mut libc::sockaddr_in6) };
                    let raw = u128::from_be_bytes(ifa_addr.sin6_addr.s6_addr);
                    let ipv6 = Ipv6Addr::from(raw);
                    ip_addrs.push(IpAddr::V6(ipv6))
                }
            };

            current = ifaddr.ifa_next as *const _;
        }

        // SAFETY: ifaddrs is still active at this point.
        unsafe { libc::freeifaddrs(ifaddrs) };

        ip_addrs
    }

    fn get_deprecated_v6_addresses(iface: &str) -> Vec<Ipv6Addr> {
        let mut addresses = Vec::new();

        // Prevent #[unused] warnings on non-Linux unixes
        let _ = iface;

        // TODO: I have no idea how to do this on BSDs.
        //
        // Also, the loop below is just a poor man's goto: there is only one
        // iteration and "break" simply means "jump to the end of this block".
        #[cfg(target_os = "linux")]
        loop {
            use std::fs::File;
            use std::io::Read;

            let Ok(mut file) = File::open("/proc/net/if_inet6") else {
                break;
            };

            let mut content = String::new();
            let Ok(_) = file.read_to_string(&mut content) else {
                break;
            };

            // Here is an example line in /proc/net/if_inet6:
            //
            // 00000000000000000000000000000001 01 80 10 80       lo
            //
            // We want the first column (the address), the 5th column (the
            // IPv6 address flags), and the final column for iface check.

            for line in content.lines() {
                let mut split = line.split_whitespace();

                // Note, this consumes an element...
                let Some(address) = split.nth(0) else {
                    continue;
                };

                // ... so that's why this is not nth(4).
                let Some(flags) = split.nth(3) else { continue };

                let Some(inet_iface) = split.nth(0) else {
                    continue;
                };

                if inet_iface.trim() != iface {
                    continue;
                }

                let Ok(address) = u128::from_str_radix(address, 16) else {
                    continue;
                };

                let Ok(flags) = u8::from_str_radix(flags, 16) else {
                    continue;
                };

                // Defined in <linux/if_addr.h>:
                const IFA_F_DEPRECATED: u8 = 0x20;

                if flags & IFA_F_DEPRECATED > 0 {
                    addresses.push(Ipv6Addr::from(address))
                }
            }

            break;
        }

        addresses
    }

    pub fn get_interface_v4_addresses(iface: &str, mask: &NetworkV4) -> Option<Ipv4Addr> {
        let mut result = None;

        for addr in transverse_ifaddr(iface) {
            match addr {
                IpAddr::V4(v4) if mask.in_range(v4) => result = Some(v4),
                _ => (),
            }
        }

        result
    }

    pub fn get_interface_v6_addresses(iface: &str, mask: &NetworkV6) -> Option<Ipv6Addr> {
        let mut result = None;

        let deprecated = get_deprecated_v6_addresses(iface);

        for addr in transverse_ifaddr(iface) {
            match addr {
                IpAddr::V6(v6) => {
                    if mask.in_range(v6) && deprecated.iter().find(|ip| **ip == v6).is_none() {
                        result = Some(v6)
                    }
                }
                _ => (),
            }
        }

        result
    }
}

#[cfg(tests)]
mod tests {
    use super::*;

    #[test]
    pub fn network_v4() {
        // This is inherently environment-dependent.
        // let mask = "192.168.1.0/24".parse::<NetworkV4>().unwrap();
        // let ip = get_interface_v4_addresses("wlan0", mask);
        // assert!(ip.is_some());
    }

    #[test]
    pub fn network_v6() {
        // This is inherently environment-dependent.
        // let mask = "fc01::/64".parse::<NetworkV6>().unwrap();
        // let ip = get_interface_v6_addresses("wlan0", mask);
        // assert!(ip.is_some());
    }
}
