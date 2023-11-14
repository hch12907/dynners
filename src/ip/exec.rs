use std::ffi::OsString;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::os::unix::prelude::OsStringExt;
use std::process::Command;

use crate::GENERAL_CONFIG;

pub(super) fn execute_command_v4(command: &str) -> Result<Ipv4Addr, String> {
    let process = Command::new(GENERAL_CONFIG.get().unwrap().shell.as_ref())
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| e.to_string())?;

    let output = OsString::from_vec(process.stdout)
        .into_string()
        .map_err(|_| String::from("got gibberish from child process"))?;

    output.trim().parse::<Ipv4Addr>().map_err(|e| e.to_string())
}

pub(super) fn execute_command_v6(command: &str) -> Result<Ipv6Addr, String> {
    let process = Command::new(GENERAL_CONFIG.get().unwrap().shell.as_ref())
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| e.to_string())?;

    let output = OsString::from_vec(process.stdout)
        .into_string()
        .map_err(|_| String::from("got gibberish from child process"))?;

    output.trim().parse::<Ipv6Addr>().map_err(|e| e.to_string())
}
