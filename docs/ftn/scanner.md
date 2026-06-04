# FTN Scanner

The FTN scanner exports local echomail messages into outbound packet files for
configured links.

Current v1.2 behavior:

- scans enabled links for the selected network profile
- reads subscribed `network_area_subscriptions`
- finds normal local/system messages in mapped echomail areas
- writes Type-2+ `.pkt` files to
  `paths.runtime/network/<profile>/outbound/<link>/ready`
- records outbound `network_packets` rows with `status = 'pending'`
- records exported `network_messages` rows
- avoids exporting the same local message to the same link more than once

Run it manually:

```bash
oxidebbs-server net scan fidonet
oxidebbs-server --json net scan fidonet
```

Current limitations:

- netmail packet creation is not wired yet
- advanced SEEN-BY/PATH loop prevention is still limited
- `oxidebbs-ftn` can create ZIP bundles, but `net scan` still writes raw `.pkt`
  files and does not yet call the bundle creator
- ARJ bundle creation is not implemented
- BinkP poll execution is responsible for transporting ready packet files and is
  a separate remaining phase

The default outbound layout for profile `fidonet` and link `hub` is:

```text
runtime/network/fidonet/
  outbound/
    hub/
      ready/
      busy/
      sent/
      hold/
      temp/
```
