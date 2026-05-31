# Contributing to OxideBBS

## Ground rules

- Keep the caller experience retro and ANSI-first.
- Keep the implementation modern and testable.
- Do not add an external database.
- Do not bundle questionable DOS door software.
- Document meaningful design decisions in ADRs.

## Local checks

```bash
./scripts/dev-check.sh
```

## Branch naming

Suggested:

```text
feature/telnet-listener
feature/door-dropfiles
fix/cp437-rendering
docs/adr-door-runner
```


## Contribution licensing

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in OxideBBS is licensed under Apache-2.0, without additional
terms or conditions.
