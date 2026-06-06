# FTN Packet Format

OxideBBS implements byte-preserving FTN Type-2 packet primitives in the
`oxidebbs-ftn` crate.

Current packet support:

- `PacketHeader` models the Type-2/Type-2+ header fields used by OxideBBS.
- `PacketReader` reads packet bytes from `impl Read`.
- `PacketWriter` writes packet bytes to `impl Write`.
- `PacketMessage` preserves raw message body bytes, including non-UTF-8 legacy
  text.
- `MessageAttribute` exposes common packed-message attribute bits.

The reader rejects unsupported packet and packed-message types. Message body
bytes remain authoritative for duplicate hashing and re-export; display text is
decoded only at higher layers.

Tosser, scanner, bundle, and nodelist workflows build on these primitives and
are tracked separately from this packet-format reference.
