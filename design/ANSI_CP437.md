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

## Screen assets

Suggested asset paths:

```text
assets/ansi/welcome.ans
assets/ansi/logon.ans
assets/ansi/logoff.ans
assets/ansi/main-menu.ans
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

## Not in scope initially

- Full ANSI editor
- RIP graphics
- Avatar protocol
- Mouse support
