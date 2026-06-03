# Security Policy

## Supported versions

OxideBBS follows Semantic Versioning.

| Version | Supported |
| --- | --- |
| `v1.1.x` | Yes |
| `v1.0.x` | Critical fixes at maintainer discretion |

Security reports against unreleased changes on `main` are accepted and handled
before the next release when applicable.

## Reporting

Report security issues privately to the repository owner. Do not open public
issues for suspected vulnerabilities until a fix or disclosure plan is ready.

## Security priorities

- Password hashing
- Safe config handling
- No shell injection in door runner
- Door sandboxing/containment where practical
- No exposure of sensitive config to callers
- Safe handling of telnet input
