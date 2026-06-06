# Netmail

Netmail uses the same FTN packet message primitives as echomail, with routing
metadata carried in control lines such as `INTL`, `FMPT`, `TOPT`, `MSGID`,
`REPLY`, `FLAGS`, and `Via`.

The runtime can parse and compose those control lines, preserve raw packet
message bodies, make local/direct/hub/crash/hold/unknown routing decisions,
deliver local netmail, queue forwarded netmail, queue AreaFix replies, and
materialize pending outbound netmail through `net scan`.

Netmail duplicate keys follow ADR 0023: MSGID is hashed with SHA-256 across the
network, origin address, destination address, and MSGID. MSGID-less messages use
the shared fallback hash policy with timestamp-bucket clock-skew candidates.

See [Netmail Routing](./netmail-routing.md) for the current routing decision
scope and runtime boundaries.
