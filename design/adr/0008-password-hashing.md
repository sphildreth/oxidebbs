# ADR 0008: Use Argon2id Password Hashing

## Status

Accepted

## Context

OxideBBS stores local user accounts in DecentDB. Password handling must be
strong enough for a network-facing telnet system while staying portable for
small sysop-run installs.

## Decision

OxideBBS will store password hashes using Argon2id encoded in the PHC string
format.

The core login flow treats password verification as a boundary dependency. It
does not compare plaintext passwords directly and does not own hash parameter
selection. The server/auth adapter verifies PHC strings and creates new PHC
hashes for telnet user registration. Local password resets currently accept a
precomputed PHC string.

## Consequences

- Passwords are never stored reversibly.
- Hash records remain self-describing because PHC strings carry algorithm,
  version, salt, and parameter metadata.
- Tests can use a verifier trait without pulling crypto behavior into pure
  domain tests.
- Future parameter upgrades can happen by detecting old PHC parameters at login
  and rehashing after successful verification.

## Rejected Options

- Plain SHA/BLAKE hashes: too fast for password storage.
- Homegrown salting: easy to get wrong and hard to audit.
- Storing plaintext for early development: not acceptable for a BBS that will
  be exposed over telnet.
