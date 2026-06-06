# Caller Command Reference

This is the sysop-facing reference for caller commands. It documents the current
runtime command surface and should be updated whenever a caller-visible command,
menu action, prompt, or default screen changes.

Screen art is not authoritative. A command is available only when it is backed by
configured menu routing or by an implemented runtime prompt.

## Default Login Menu

The starter login menu is configured by `[menus.login]`.

| Key | Label | Action | Status |
| --- | --- | --- | --- |
| `L` | Logon | `login` | Active |
| `N` | New User | `new_user` | Active |
| `G` | Goodbye | `logoff` | Active |

## Default Main Menu

The starter main menu is configured by `[menus.main]`.

| Key | Label | Action | Status |
| --- | --- | --- | --- |
| `D` | Doors | `doors` | Active |
| `M` | Messages | `messages` | Active |
| `F` | Files | `files` | Active when file transfers and file areas are configured |
| `S` | Sysop | `submenu` -> `sysop` | Active for level `255` callers |
| `G` | Goodbye | `logoff` | Active |

The default `S` item is gated by `min_security_level = 255`. Default new callers
at level `10` receive an access-denied line and remain at the main menu.

## Default Sysop Submenu

The starter sysop submenu is configured by `[menus.sysop]` and is reachable only
through the gated main-menu `S` item.

| Key | Label | Action | Status |
| --- | --- | --- | --- |
| `N` | Node Status | `noop` | Active |
| `R` | Return to main | `submenu` -> `main` | Active |
| `G` | Goodbye | `logoff` | Active |

The caller sysop submenu is intentionally non-mutating. It can show safe
sysop-oriented information, but remote mutations remain outside the caller menu
surface.

## Door Menu

The `doors` action opens the live door selector for authenticated callers.

| Input | Behavior | Status |
| --- | --- | --- |
| Door key | Launches the enabled configured door with that key | Active |
| Door number | Launches the enabled configured door at that displayed list number | Active |
| Blank line | Returns to the main menu | Active |

Disabled doors and doors that fail runtime validation are not launchable by
callers.

## Message Menu

The `messages` action opens message area selection for authenticated callers.

| Input | Behavior | Status |
| --- | --- | --- |
| Area key | Enters the enabled message area with that key | Active |
| Blank line | Returns to the main menu | Active |

Inside a message area, callers use these prompt commands:

| Key | Label | Behavior | Status |
| --- | --- | --- | --- |
| `R` | Read | Prompts for a message number and displays it | Active |
| `P` | Post | Prompts for subject and multi-line body, then posts a message | Active |
| `Y` | Reply | Prompts for a message number and multi-line body, then posts a reply | Active |
| `B` | Back | Returns to area selection | Active |
| Blank line | Back | Returns to area selection | Active |

Message subjects and bodies must be CP437-compatible before they are stored.

## File Menu

The `files` action opens file-area selection for authenticated callers when
`[file_transfers].enabled = true`.

| Input | Behavior | Status |
| --- | --- | --- |
| Area number | Enters the enabled file area | Active |
| Blank line | Returns to the main menu | Active |

Inside a file area, callers use these prompt commands:

| Key | Label | Behavior | Status |
| --- | --- | --- | --- |
| `D` | Download | Prompts for a file number and protocol, then sends the file | Active |
| `U` | Upload | Prompts for a protocol and receives a pending upload | Active |
| `R` | Return | Returns to area selection | Active |
| Blank line | Return | Returns to area selection | Active |

Supported caller transfer protocols are ZMODEM (`Z`) and XMODEM (`X`).
Uploads are stored pending sysop review.

## Supported Menu Actions

Menu items map single ASCII keys to safe internal actions. Keys route
case-insensitively.

| Action | Runtime behavior | Status |
| --- | --- | --- |
| `login` | Runs the login flow | Active |
| `new_user` | Runs new-user registration before login | Active |
| `doors` | Opens the door selector | Active |
| `messages` | Opens message area selection | Active |
| `files` | Opens file area selection | Active |
| `logoff` | Disconnects with a goodbye message | Active |
| `show_screen` | Displays a configured screen and returns to the current menu | Active |
| `submenu` | Moves into the configured target menu | Active |
| `noop` | Accepts the key and performs no visible action | Active |

## Custom Or Future Commands

No caller key is globally reserved by the router. Future commands become real
only when the config maps a key to an implemented action or prompt handler.

When changing caller commands on a board:

1. Add or update the menu action or runtime prompt.
2. Update the default config if the key is part of the starter command set.
3. Update every starter screen asset that displays the command.
4. Keep a local note of any board-specific keys so callers and co-sysops know
   what is available.
5. Treat unconfigured keys as unavailable, even if an ANSI screen mentions them.

## Source Of Truth

The active default menu keys live in `config/oxidebbs.example.toml`. The runtime
routes configured menu keys through the menu router and handles door, message,
and file prompt commands in the caller session flow.
