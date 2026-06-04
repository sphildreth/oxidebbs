# FTN Bundles

FTN mail can arrive as a raw `.pkt` file or as an arcmail archive containing
one or more packet files. OxideBBS keeps packet parsing separate from archive
handling so the tosser can make clear decisions about pass-through, extraction,
archive, and quarantine behavior.

Current `oxidebbs-ftn` bundle foundation:

- `.pkt` is classified as a raw packet and can be handed directly to
  `PacketReader`.
- `.zip` is classified as a ZIP arcmail bundle and extracts top-level `.pkt`
  entries into the requested output directory.
- `.arj` is classified as an ARJ arcmail bundle.
- ARJ extraction returns an explicit unsupported-extraction error until the
  policy is implemented.

ZIP extraction is intentionally strict. OxideBBS accepts only top-level `.pkt`
entries. It rejects nested paths, absolute or traversal-style names, non-packet
entries, duplicate output names, corrupt archives, empty archives, and attempts
to overwrite an existing extracted packet. This keeps inbound processing
deterministic before the tosser decides whether to import, archive, or
quarantine a packet.

Classic day-of-week arcmail names such as `.su0`, `.mo0`, `.tu0`, `.we0`,
`.th0`, `.fr0`, and `.sa0` still need a documented naming and compression
policy before OxideBBS treats them as extractable bundles. The current
classifier intentionally recognizes only unambiguous suffixes.

Remaining work:

- decide whether ARJ support is built in, external-tool based, or deferred
- implement outbound bundle creation and naming
- connect bundle extraction to the tosser quarantine/archive workflow
