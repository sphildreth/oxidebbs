# Netmail

Netmail uses the same FTN packet message primitives as echomail, with routing
metadata carried in control lines such as `INTL`, `FMPT`, `TOPT`, `MSGID`,
`REPLY`, `FLAGS`, and `Via`.

The current v1.2 foundation can parse and compose those control lines, preserve
raw packet message bodies, and make pure local/direct/hub/crash/hold/unknown
routing decisions through `oxidebbs-ftn`. AreaFix replies, outbound queue
handling, and scanner/tosser integration remain separate workflow phases in the
release plan.

Netmail duplicate keys follow ADR 0023: MSGID is hashed with SHA-256 across the
network, origin address, destination address, and MSGID. MSGID-less messages use
the shared fallback hash policy with timestamp-bucket clock-skew candidates.

See [Netmail Routing](./netmail-routing.md) for the current routing decision
scope and runtime boundaries.
