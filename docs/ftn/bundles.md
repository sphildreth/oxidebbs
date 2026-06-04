# FTN Bundles

FTN mail can arrive as a raw `.pkt` file or as an arcmail archive containing
one or more packet files. OxideBBS keeps packet parsing separate from archive
handling so the tosser can make clear decisions about pass-through, extraction,
archive, and quarantine behavior.

Current `oxidebbs-ftn` bundle foundation:

- `.pkt` is classified as a raw packet and can be handed directly to
  `PacketReader`.
- `.zip` is classified as a ZIP arcmail bundle.
- `.arj` is classified as an ARJ arcmail bundle.
- ZIP and ARJ extraction return explicit unsupported-extraction errors until
  decompression support is implemented.

Classic day-of-week arcmail names such as `.su0`, `.mo0`, `.tu0`, `.we0`,
`.th0`, `.fr0`, and `.sa0` still need a documented naming and compression
policy before OxideBBS treats them as extractable bundles. The current
classifier intentionally recognizes only unambiguous suffixes.

Remaining work:

- implement ZIP extraction without path traversal
- decide whether ARJ support is built in, external-tool based, or deferred
- implement outbound bundle creation and naming
- connect bundle extraction to the tosser quarantine/archive workflow
