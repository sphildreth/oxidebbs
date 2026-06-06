# FTN Scanner

The FTN scanner exports local echomail messages into outbound packet files for
configured links.

Current behavior:

- scans enabled links for the selected network profile
- reads subscribed `network_area_subscriptions`
- finds normal local/system messages in mapped echomail areas
- writes Type-2+ `.pkt` files to
  `paths.runtime/network/<profile>/outbound/<link>/ready`
- records outbound `network_packets` rows with `status = 'pending'`
- records exported `network_messages` rows
- avoids exporting the same local message to the same link more than once
- materializes pending outbound netmail rows into `.pkt` files for the target
  link
- bundles ready packet files into pending ZIP arcmail rows for links configured
  with ZIP compression

Run it manually:

```bash
oxidebbs-server net scan fidonet
oxidebbs-server --json net scan fidonet
```

Runtime notes:

- BinkP poll execution transports pending outbound packet or bundle rows and
  marks them processed after acknowledgement
- outbound arcmail creation uses ZIP; inbound ARJ extraction is supported by the
  tosser
- advanced SEEN-BY/PATH loop prevention remains intentionally conservative

The default outbound layout for profile `fidonet` and link `hub` is:

```text
runtime/network/fidonet/
  outbound/
    hub/
      ready/
      bundled/
```
