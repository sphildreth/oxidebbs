# FTN Tosser

The FTN tosser imports inbound legacy packet files into local message areas.

Current v1.2 behavior:

- scans `paths.runtime/network/<profile>/inbound/drop`
- accepts raw `.pkt` files
- extracts ZIP bundles that contain only top-level `.pkt` entries
- extracts ARJ bundles that contain only top-level `.pkt` entries
- validates packet origin address and packet password against enabled links
- imports known echomail `AREA:` messages into mapped local message areas
- delivers local netmail to the configured local netmail area
- queues direct or hub-routed non-local netmail for outbound materialization
- processes inbound AreaFix netmail, queues reply netmail, and queues targeted
  per-area/per-link rescan requests
- stores imported metadata in `network_messages`
- stores packet status in `network_packets`
- stores parsed `SEEN-BY` and `PATH` nodes for imported echomail
- skips duplicate echomail by MSGID/body hash and logs the rejection
- moves successful input files to `inbound/archive`
- moves malformed or unauthorized input files to `inbound/quarantine`

Run it manually:

```bash
oxidebbs-server net toss fidonet
oxidebbs-server --json net toss fidonet
```

The network profile must be enabled. Inbound packet passwords are compared
case-insensitively with the matching link password.

Runtime notes:

- unknown echomail AREA tags are quarantined instead of auto-created
- malformed, unauthorized, corrupt, or unsupported inbound files are quarantined
  with packet/message state for operator review
- scanner/export and BinkP poll commands are separate operator-invoked steps

The default spool layout for profile `fidonet` is:

```text
runtime/network/fidonet/
  inbound/
    drop/
    archive/
    quarantine/
  temp-inbound/
```
