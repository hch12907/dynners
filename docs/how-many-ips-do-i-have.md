# How many IP addresses do you have?

Here is my `ip addr` output, with the addresses sorted. 

```bash
$ ip -o ad show | while read index iface inet addr REST; do echo $iface $addr; done | sort
enp1s0 192.168.0.233/24
enp1s0 2xxx:xxxx:xxxx:xxxx:XXXX:XXXX:XXXX:XXXX/64
enp1s0 2xxx:xxxx:xxxx:xxxx:YYYY:YYYY:YYYY:YYYY/64
enp1s0 2xxx:xxxx:xxxx:xxxx:ZZZZ:ZZZZ:ZZZZ:ZZZZ/64
enp1s0 2xxx:xxxx:xxxx:xxxx:WWWW:WWWW:WWWW:WWWW/64
enp1s0 fd01:<snip>/64
enp1s0 fe80::<snip>/64
```

The `xxxx` part of the addresses is dynamic and comes from the Router Advertisement.
The capitalised parts of the address (`XXXX`, `YYYY`, `ZZZZ`, `WWWW`) are static. In
the config, I match on them using `2000::XXXX:XXXX:XXXX:XXXX/e000::ffff:ffff:ffff:ffff`.
