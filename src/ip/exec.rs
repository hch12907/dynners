use std::ffi::OsString;
use std::net::AddrParseError;
use std::os::unix::prelude::OsStringExt;
use std::process::Command;
use std::str::FromStr;

use crate::GENERAL_CONFIG;

pub(super) fn execute_command_for_ip<T>(command: &str) -> Result<T, String>
where
    T: FromStr<Err = AddrParseError>,
{
    let process = Command::new(GENERAL_CONFIG.get().unwrap().shell.as_ref())
        .arg("-c")
        .arg(command)
        .output()
        .map_err(|e| e.to_string())?;

    let output = OsString::from_vec(process.stdout)
        .into_string()
        .map_err(|_| String::from("got gibberish from child process"))?;

    output.trim().parse::<T>().map_err(|e| e.to_string())
}
