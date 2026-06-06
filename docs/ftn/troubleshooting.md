# FTN Troubleshooting

Use the boundary where the failure appears to decide what to inspect first.

| Symptom | Check |
| --- | --- |
| Local posts do not create outbound files | Area subscription state, scanner output, message visibility, and outbound queue rows. |
| `net poll` cannot connect | Link host, BinkP port, firewall, DNS, and whether the remote requires TLS. |
| TLS-required poll fails | Remote certificate validity, hostname match, system trust roots, and whether the remote actually speaks TLS on the configured port. |
| TLS-opportunistic poll falls back to plaintext | The remote did not complete TLS. Confirm this is expected before relying on legacy fallback. |
| Inbound TLS clients fail immediately | `[network.binkp_listener]` must include `tls_cert_path` and `tls_key_path`, and the files must be readable by the server process. |
| Inbound plaintext clients are refused | The authenticated link likely has `transport_security = "tls_required"`. Use TLS or change the link policy deliberately. |
| Password authentication fails | Compare the configured link address and password on both systems. BinkP password errors do not echo secrets. |
| Files arrive but messages do not appear | Run `net toss <network>` and inspect packet/message quarantine state. |
| Outbound files remain pending | Check `net queue <link>`, `net logs <link>`, and whether the peer acknowledges files with `M_GOT`. |

Useful commands:

```sh
oxidebbs-server net poll <link> --dry-run
oxidebbs-server net poll <link>
oxidebbs-server net queue <link>
oxidebbs-server net logs <link>
oxidebbs-server net packets summary
oxidebbs-server net packets show <packet-id>
```

Plaintext legacy mode exposes credentials and mail contents to the network. Use
it only for legacy FTN peers that cannot use TLS.
