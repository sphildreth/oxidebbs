# Menu System

OxideBBS separates caller-facing screen art from safe menu actions.

Screen files draw what the caller sees. Menu config decides which single key
invokes which internal action.

Terminal welcome art should stay informational: branding, system identity, and
status only. Command lists belong on the active menu screen whose configured
items are being routed at that prompt.

## Caller Flow

The configured starter flow is:

```text
connect
  -> choose login screen asset for caller capabilities
  -> route login menu key
  -> after login, show optional post-login screens
  -> choose main menu screen asset
  -> route main menu key
```

Screen asset selection prefers the best available variant:

1. 40-column ANSI when the caller supports ANSI and width is 40 or less
2. regular ANSI when the caller supports ANSI
3. ASCII
4. plain text

## Config Shape

```toml
[flow]
login_screen = "login"
login_menu = "login"
post_login_screens = ["screen1", "screen2"]
main_menu = "main"

[screens.login]
ansi = "login/login.ans"
ansi_40 = "login/login-40.ans"
ascii = "login/login.asc"
text = "login/login.txt"

[menus.login]
screen = "login"
prompt = "Login? "

[[menus.login.items]]
key = "L"
label = "Logon"
action = "login"
```

Menu actions are safe internal actions, not shell commands. Supported starter
actions include `login`, `new_user`, `doors`, `messages`, `logoff`, `noop`,
`show_screen`, and `submenu`.

`submenu` actions are now runtime-capable: when selected, the caller moves into
the referenced menu and continues from that menu context.

See the [Caller Command Reference](./caller-commands.md) for the current
sysop-facing list of default caller keys, prompt commands, and future/reserved
command notes.

## Asset Layout

Starter caller screens live under `assets/screens/`:

```text
assets/screens/
├── login/
├── info/
└── menus/
    └── main/
```

This keeps login screens, post-login information screens, and menu graphics
separate from legacy/general ANSI files in `assets/ansi/`.
