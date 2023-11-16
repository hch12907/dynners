# dynners

A DDNS update client written in Rust that can automatically sync your current IP
addresses (IPv4 and IPv6 alike) to the DNS. It aims to be extensible while being
lightweight enough in terms of both memory usage and binary size.

A key difference between this client and other clients is that dynners does not
make the assumption that each internet-connected device has only one IPv4 and
possibly one IPv6 address. Instead, you tell dynners to gather a list of IP addresses
and assign them to the DDNS providers as how you like it.

This allows me to run different services on different randomized IPv6 addresses,
so that someone who knows for instance the IP address to my HTTP server, would
(presumably) not also know the IP address to my SSH server.

## Supported providers
Currently, the following DDNS providers are supported:

* Cloudflare
* DNS-O-Matic
* DuckDNS
* Dynu
* IPv64
* NoIP
* Porkbun
* selfHOST.de

## Building
```bash
# Without regex
$ cargo build --release

# With regex, the binary will be more heavyweight with this enabled (~1.2MB increase)
$ cargo build --release --features regex

# For installation, a simple mv or cp is enough. 
# You might want to install a systemd service though.
$ sudo mv ./target/release/dynners /usr/local/bin/
```

The list may not be up to date. See the `src/services` directory or the sample
config.toml for an up-to-date list.

## Usage
`dynners` does not have a CLI implementation. To use it, a config file must be provided.
The file [config.toml](./examples/config.toml) located at the root of this repository is
a good starting point.

The simplest configuration file will look something like this:

```toml
[general]
   update_rate = 300 # update every 300 seconds

[ip.my-personal-ipv4]
   version = 4
   method = "http"   # grab your public IP from the web
   url = "https://myip.dnsomatic.com/"

[ddns.my-duck-dns]
   service = "duckdns"
   ip = "my-personal-ipv4"
   token = "your-token"
   domains = "example.duckdns.org"
```

## Development
Dynners is primarily developed for Linux, BSD, and other Unixes, but nothing except
development time really prevents it from supporting Windows and other platforms.

Other than that, it would be cool to support more DDNS providers.

Pull requests welcome! (Note that the program tries really hard to be unwrap()-free,
almost every error is intended to be recoverable.)

# License

This project is GPLv2 licensed.

