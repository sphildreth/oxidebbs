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
- `BundleCreator::create_zip_bundle` creates a ZIP arcmail bundle from one or
  more packet files using top-level packet filenames as entries.
- `.arj` is classified as an ARJ arcmail bundle and extracts top-level `.pkt`
  entries through the built-in archive reader.

ZIP extraction is intentionally strict. OxideBBS accepts only top-level `.pkt`
entries. It rejects nested paths, absolute or traversal-style names, non-packet
entries, duplicate output names, corrupt archives, empty archives, and attempts
to overwrite an existing extracted packet. This keeps inbound processing
deterministic before the tosser decides whether to import, archive, or
quarantine a packet.

ZIP creation is also intentionally strict. OxideBBS refuses empty packet lists,
non-`.pkt` packet inputs, duplicate packet filenames case-insensitively, and
existing bundle output files. The created bundle can be passed back through the
same extraction boundary for verification.

`net scan` calls the ZIP bundle creator for enabled links configured with ZIP
compression. It records the created bundle as a pending outbound packet row and
marks the individual packet rows processed with a bundled reason.

Classic day-of-week arcmail names such as `.su0`, `.mo0`, `.tu0`, `.we0`,
`.th0`, `.fr0`, and `.sa0` still need a documented naming and compression
policy before OxideBBS treats them as extractable bundles. The current
classifier intentionally recognizes only unambiguous suffixes.

Out of scope for caller-facing FTN operations:

- implement arcmail bundle naming
- outbound ARJ bundle creation; outbound arcmail creation is ZIP-only
