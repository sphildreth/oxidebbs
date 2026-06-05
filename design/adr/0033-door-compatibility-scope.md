# ADR 0033: Door Compatibility Scope

## Status

Accepted

## Context

OxideBBS is a BBS engine that should run the large existing ecosystem of BBS
doors. The project already supports local DOS door execution through DOSEMU2,
drop-file generation, runtime isolation, remote door-provider integration, and
credential redaction.

Repeated planning passes kept reintroducing a project-owned door-authoring API
or SDK as future work. That is not the product direction. The project is not
about creating new door games or owning a new door-development ecosystem.

## Decision

OxideBBS will not create or maintain a project-owned door-authoring API, SDK, or
framework.

Door work remains focused on compatibility with existing doors:

- local 16-bit DOS door execution through the current DOSEMU2 path
- byte-exact drop-file compatibility for common BBS door formats
- terminal and CP437/ANSI behavior expected by existing door programs
- sysop configuration, validation, dry-run, audit, and troubleshooting tools
- remote door-provider compatibility where the provider is an existing service

Future release plans must not add a door-authoring API, SDK, framework, sample
new-door project, or crate for new door development unless this ADR is first
superseded by a maintainer-approved ADR.

## Consequences

- v1.3 door scope is compatibility work, especially drop-file coverage and
  operator tooling.
- Source-available door projects may be used as behavior references or
  compatibility tests, but not as a reason to build a new authoring platform.
- The existing DOS-door and remote-provider boundaries stay the supported door
  model.
- Documentation should describe how to run existing doors, not how to create a
  new OxideBBS-specific door ecosystem.
