# ADR 0034: PETSCII Translation And Terminal-Profile Persistence Policy

## Status

Accepted

## Context

OxideBBS documents C64 / C64 Ultimate / PETSCII-friendly 40-column callers as a
supported terminal profile, but until now the C64 profile only provided an
ASCII fallback charset (`PetsciiAsciiFallback`). Box-drawing and high-bit
glyphs were never translated to PETSCII, so a real C64 caller received CP437 or
UTF-8 bytes its character set could not render.

`design/RELEASE_v1_3_PLAN.md` phase P2 requires full PETSCII encode/decode
rendering beyond the ASCII fallback, and phase P3 requires persisting a manual
terminal-profile preference. Both need a documented translation and persistence
policy so implementation does not invent ad-hoc behavior.

The caller UI remains byte-oriented (not Unicode-first). Output text is built as
Unicode `String` inside the server, then encoded to wire bytes by the terminal's
charset. Binary file-transfer data and telnet negotiation bytes must never be
re-encoded.

## Decision

### PETSCII character set and translation

- Implement the C64 "upper case and graphics" PETSCII screen set in
  `oxidebbs-term` as a full 256-entry decode table (`decode_petscii`) plus a
  reverse encode path (`encode_petscii`, `render_petscii`, `render_petscii_lossy`).
- Printable ASCII (`0x20`-`0x7E`) maps to itself. Letters are accepted in the
  unshifted (`0x41`-`0x5A`, `0x61`-`0x7A`) and shifted (`0xC1`-`0xDA`,
  `0xE1`-`0xFA`) PETSCII ranges. `0x0D` and `0x0A` decode to a logical newline.
- The `0xA0`-`0xDF` graphics range carries the standard C64 line-drawing and
  block glyphs (corners, tees, crosses, shade and half blocks). Glyph fidelity
  for the graphics range follows C64 ROM approximations; the exact visual on a
  given emulator is not guaranteed.
- Unsupported source characters use a replacement policy: lossy encoding
  replaces them with `?`. Strict encoding (`render_petscii`) returns
  `PetsciiEncodeError` so callers that must never fail can choose lossy.

### Charset selection

- Add `TerminalCharset::Petscii` (config string `"petscii"`) as the real
  PETSCII charset. `PetsciiAsciiFallback` (`"petscii_ascii_fallback"`) remains
  a supported value for operators that want the historical ASCII-only behavior.
- The built-in C64 profile (`TerminalCapabilities::c64()`) and the default
  generated/example C64 config now select `Petscii`.

### Routing

- Caller output text is charset-aware at the central `encode_text_into`
  chokepoint. When the charset is `Petscii`, text is encoded to PETSCII bytes;
  otherwise CP437/ASCII behavior is unchanged for ANSI and plain callers.
- Binary file-transfer writes and telnet IAC negotiation bytes bypass text
  encoding and are never PETSCII-converted, so transfers and negotiation remain
  intact for C64 callers.

### Manual profile persistence (P3 scope)

- A persisted terminal-profile preference is the highest-priority source when
  present, overriding unreliable telnet detection. The fallback order is:
  persisted user preference > telnet terminal-type detection > configured
  default profile.
- Existing users migrate with no forced preference; detection behaves exactly
  as before until a preference is set.
- The user/account schema migration, onboarding flow, and sysop edit support
  are tracked under P3 and are not required for P2's PETSCII rendering.

## Consequences

- C64 callers now receive PETSCII-encoded text for menus, messages, file lists,
  and logoff flow. Existing ANSI/CP437 and plain-ASCII snapshots are unchanged.
- `PetsciiAsciiFallback` remains available but is no longer the C64 default;
  operators that depended on it must set `charset = "petscii_ascii_fallback"`.
- Text that contains glyphs with no PETSCII representation is lossily replaced
  with `?` rather than failing the caller session.
- Profile persistence work can proceed independently in P3 using the fallback
  order above.
