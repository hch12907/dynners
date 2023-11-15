# dynners

A DDNS update client written in Rust.

## Building
```bash
# Without regex
$ cargo build --release

# With regex, the binary will be more heavyweight with this enabled (~1.2MB increase)
$ cargo build --release --features regex
```

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

The list may not be up to date. See the `src/services` directory or the sample
config.toml for an up-to-date list.

## Usage
`dynners` does not have a CLI implementation. To use it, a config file must be provided.
The file [config.toml](./config.toml) located at the root of this repository is a good starting point.

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

# License

This project is GPLv2 licensed.

