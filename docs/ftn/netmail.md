# Netmail

Netmail uses the same FTN packet message primitives as echomail, with routing
metadata carried in control lines such as `INTL`, `FMPT`, `TOPT`, `MSGID`,
`REPLY`, `FLAGS`, and `Via`.

The current v1.2 foundation can parse and compose those control lines and can
preserve raw packet message bodies. Netmail routing, hub decisions, AreaFix
replies, and outbound queue handling are separate workflow phases in the release
plan.

Netmail duplicate keys follow ADR 0023: MSGID is hashed with SHA-256 across the
network, origin address, destination address, and MSGID. MSGID-less messages use
the shared fallback hash policy with timestamp-bucket clock-skew candidates.
