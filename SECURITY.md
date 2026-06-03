# Security Policy

## Supported versions

OxideBBS follows Semantic Versioning.

- `v1.0.x` receives security fixes for the current released line.
- The `main` branch tracks vnext work for `v1.1.0`; security reports against
  unreleased changes are accepted and handled before release when applicable.

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
