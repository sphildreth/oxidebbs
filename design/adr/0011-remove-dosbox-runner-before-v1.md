# ADR 0011: Remove DOSBox Runner Before V1

## Status

Accepted

## Context

OxideBBS is still in early development. There is no v1 compatibility promise
for door runner configuration, scripts, or runtime behavior.

The project briefly supported a DOSBox-based proof path for live doors. That
path introduced DOSBox-specific command planning, a DOSBox nullmodem TCP serial
bridge, quiet DOSBox configuration, and an optional Xvfb wrapper for headless
hosts.

Keeping DOSBox and DOSEMU2 as parallel v1 runtimes would increase the amount of
code and documentation that must be tested before the project has a stable door
contract. It would also keep the weaker headless story visible to sysops even
though the project has decided that DOSEMU2 is the intended server runtime.

## Decision

Remove DOSBox support before v1 instead of maintaining parallel DOSBox and
DOSEMU2 runners.

The refactor must remove active DOSBox runtime code, scripts, examples, and
operator documentation. Historical changelog entries and ADR context may still
mention DOSBox as the removed earlier approach.

The supported v1 door runtime will be DOSEMU2 only.

## Consequences

- The v1 door contract is simpler.
- Sysop documentation has one installation and troubleshooting path.
- Tests have one canonical DOS runtime path.
- Implementation effort moves toward the runtime intended for headless Linux
  containers instead of preserving an early proof-of-concept path.
- Existing pre-v1 local configs that use `runner = "dosbox"` will need to be
  updated to `runner = "dosemu"` or an absolute DOSEMU2 path.
- The project loses DOSBox as a fallback runner unless a future ADR reintroduces
  it for a clearly defined compatibility reason.

## References

- Refactor plan:
  `design/DOSBOX_TO_DOSEMU2_REFACTOR_PLAN.md`
- DOSEMU2 runtime ADR:
  `design/adr/0010-use-dosemu2-for-dos-door-runtime.md`
