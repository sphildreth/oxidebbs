# ADR 0007: Use GitHub as Canonical Repository and Apache-2.0 License

## Status

Accepted

## Context

OxideBBS is intended to attract outside contributors. GitHub has the largest contributor reach, excellent issue/PR workflows, mature CI, and strong compatibility with coding-agent workflows.

The project also needs a standard open-source license that is permissive, familiar, and contributor-friendly.

## Decision

Use GitHub as the canonical repository host.

Use the Apache License, Version 2.0.

Unless explicitly stated otherwise, any contribution intentionally submitted
for inclusion in OxideBBS is licensed under Apache-2.0 without additional terms
or conditions.

Project-created docs, examples, ANSI assets, and test fixtures in this
repository are covered by Apache-2.0 unless a file explicitly says otherwise.

Do not bundle old DOS doors, shareware packages, abandonware, or third-party
assets unless their license clearly allows redistribution by OxideBBS.

A Codeberg mirror is now available, but GitHub remains the source of truth unless this ADR is superseded.

## Consequences

- Easier contributor onboarding.
- Better discoverability.
- Better compatibility with GitHub Actions and AI coding agents.
- Apache-2.0 provides a permissive license with an explicit patent grant.
- Sysops can freely run, modify, and distribute OxideBBS under Apache-2.0 terms.
- Door support can document third-party setup, but repository contents must stay
  clear of unclear redistribution rights.
