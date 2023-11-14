# dynners

A DDNS update client written in Rust.

## Building
```bash
# Without regex
$ cargo build --release

# With regex, the binary will be more heavyweight with this enabled (~1.2MB increase)
$ cargo build --release --features regex
```

## Usage
`dynners` does not have a CLI implementation. To use it, a config file must be provided.
The file [config.toml](./config.toml) located in the root of this repository is a good starting point,
but the general gist is:

1. Setup the `[ip]` table. This dictates how the program obtains IP addresses that will
   be used to configure the DNS records.

2. Setup the `[ddns]` table.

3. Run the program.

# License

This is GPLv2 licensed.

