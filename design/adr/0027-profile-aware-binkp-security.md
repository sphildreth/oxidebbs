# ADR 0027: Profile-Aware BinkP Transport Security

## Status

Accepted

## Context

BinkP is required for real network mail exchange. Legacy FTN links commonly use
plaintext BinkP, while OxideNet and new private network profiles should not
normalize plaintext credentials and message exchange.

## Decision

BinkP security is configured per link with these values:

- `tls_required`
- `plaintext_legacy`
- `tls_opportunistic`

OxideNet and new private network profiles default to `tls_required`.

Legacy FTN links may use `plaintext_legacy` only when the link explicitly opts
in. Startup and poll logs must warn that reusable BinkP passwords and message
contents are exposed.

`tls_opportunistic` is included in v1.2. It attempts TLS first and may fall back
to plaintext only for links that are also marked as legacy-compatible.

## Consequences

- Secure defaults apply to OxideNet.
- Real legacy FTN interoperability remains possible.
- Operators get clear warnings for plaintext links.
- Tests must cover TLS success, TLS failure, plaintext opt-in, and
  opportunistic fallback.
