# FTN Configuration

FTN transport is configured under `[network]` in `oxidebbs.toml`.

Enable the network subsystem, define one or more profiles, then define links
for the hubs or peers you poll or accept inbound sessions from.

```toml
[network]
enabled = true

[network.binkp_listener]
enabled = true
bind = "0.0.0.0:24554"
max_connections = 10
tls_cert_path = "./certs/binkp.crt"
tls_key_path = "./certs/binkp.key"

[network.profiles.fidonet]
name = "FidoNet"
enabled = true
adapter = "legacy-ftn"

[network.profiles.fidonet.local_address]
zone = 1
net = 105
node = 42
point = 0

[network.links.fidonet_hub]
network = "fidonet"
address = "1:105/0"
host = "fidonet.example.net"
binkp_port = 24554
password = "shared-secret"
poll_schedule_minutes = 60
enabled = true
compression = "zip"
transport_security = "tls_opportunistic"
legacy_compatible = true
```

`transport_security` accepts:

- `tls_required`: TLS is mandatory. Outbound polls fail when TLS or certificate
  validation fails. Inbound plaintext sessions for the link are rejected.
- `tls_opportunistic`: TLS is attempted first. Plaintext fallback is allowed
  only when `legacy_compatible = true`.
- `plaintext_legacy`: plaintext BinkP is allowed only for `legacy-ftn`
  profiles and should be used only for legacy peers that cannot use TLS.

The listener needs `tls_cert_path` and `tls_key_path` when it is enabled and any
enabled link requires TLS. The certificate file is a PEM certificate or chain;
the key file is a PEM PKCS#8 private key.

`net poll --dry-run <link>` shows the transport-security plan without opening a
socket. Use it after changing link policy.
