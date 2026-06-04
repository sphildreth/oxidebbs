# FTN Tosser

The FTN tosser imports inbound legacy packet files into local message areas.

Current v1.2 behavior:

- scans `paths.runtime/network/<profile>/inbound/drop`
- accepts raw `.pkt` files
- extracts ZIP bundles that contain only top-level `.pkt` entries
- rejects ARJ bundles with an explicit unsupported-extraction error
- validates packet origin address and packet password against enabled links
- imports known echomail `AREA:` messages into mapped local message areas
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

Current limitations:

- netmail is recorded as quarantined metadata until local delivery and forwarding
  are wired through the routing phase
- unknown echomail AREA tags are quarantined instead of auto-created
- scanner/export, ARJ extraction, arcmail bundle creation, and BinkP poll loops
  are separate remaining phases

The default spool layout for profile `fidonet` is:

```text
runtime/network/fidonet/
  inbound/
    drop/
    archive/
    quarantine/
  temp-inbound/
```
