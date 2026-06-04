# ANSI and CP437 Design Notes

## Goal

OxideBBS should feel like a real BBS, not a web app pretending to be one.

That means ANSI/CP437 rendering is core infrastructure.

## Principles

1. Keep caller output byte-oriented.
2. Preserve CP437 box drawing.
3. Support classic ANSI colors.
4. Do not assume Unicode terminals.
5. Test with SyncTERM early.
6. Keep screen assets as `.ans` files where practical.
7. Treat 40-column terminals as a supported caller profile.
8. Treat C64/C64 Ultimate callers as a supported compatibility profile, not as
   a C64-native OxideBBS runtime.

## Screen assets

Suggested asset paths:

```text
assets/ansi/welcome.ans
assets/ansi/welcome.asc
assets/ansi/welcome-40.ans
assets/ansi/logon.ans
assets/ansi/logoff.ans
assets/ansi/logoff.asc
assets/ansi/main-menu.ans
assets/ansi/main-menu-40.ans
assets/ansi/sysop-menu.ans
```

## Renderer responsibilities

- Clear screen
- Move cursor
- Set foreground/background color
- Reset attributes
- Write raw bytes
- Page long output
- Render menu prompts
- Select width-specific assets when available
- Keep prompts, status bars, wrapping, and paging usable at 40 columns
- Route C64/plain callers to ASCII or PETSCII-friendly fallbacks and avoid
  ANSI-only navigation.

## C64/PETSCII-friendly profile

The `c64` terminal profile represents callers using C64, C64 Ultimate, or C64
terminal applications. Its default geometry is 40 x 25, it does not assume ANSI
escape support, and it uses ASCII/PETSCII-friendly fallback rendering for basic
navigation.

Full PETSCII translation remains future work. Until that exists, C64 callers
must still be able to log in, choose menus, read message lists and messages, see
file lists, and log off through plain 40-column assets or generated text that
fits the active width.

## Not in scope initially

- Full ANSI editor
- RIP graphics
- Avatar protocol
- Mouse support
- Running the OxideBBS server binary on C64 hardware
