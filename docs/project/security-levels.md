# User Security Levels

OxideBBS stores a numeric `security_level` on each user account. The valid range
is `0..255`; higher numbers represent more local access.

Security levels are a sysop policy tool, not a fixed role system. OxideBBS v1 has
only a few enforced gates, and values between those gates are available for the
sysop to define.

## Current Defaults

| Scope | Value | Meaning |
| --- | --- | --- |
| New caller | `10` | Default from `[auth].new_user_security_level` |
| Default message-area read level | `0` | Any active caller can read normal messages in that area |
| Default message-area post level | `10` | Default new callers can post in starter local areas |
| Setup-created sysop | `255` | Initial sysop account created by `setup` |
| Promoted sysop | `255` | `users promote-sysop` also sets `is_sysop = true` |

The generated config uses:

```toml
[auth]
new_user_security_level = 10
```

Changing that value affects future new-user registrations. It does not
retroactively update existing users.

## What Is Enforced

Current runtime enforcement is intentionally small:

- New-user registration stores the configured `[auth].new_user_security_level`.
- User creation through the CLI accepts an explicit `--level`.
- Message reading is gated by each area's `read_security_level`.
- Message posting and replies are gated by each area's `post_security_level`.
- `users promote-sysop` sets `is_sysop = true` and raises the user to level
  `255`.

The sysop flag is separate from the numeric level. Do not treat `255` alone as a
complete sysop identity; use `users promote-sysop` when the account should be a
sysop. `users demote-sysop` clears the sysop flag, but it does not lower the
numeric security level. Run `users set-level` after demotion when the account
should also lose level `255`.

## Enforced Since v1.2

These areas now use user security levels since v1.2:

- Door launching from the caller `Doors` menu: if `min_security_level` is set on a door definition, callers with lower security levels are denied access. The effective door launch level is the maximum of the invoking menu item level and the door definition level.
- Caller menu routing: menu items with `min_security_level` above the authenticated user's level are rejected with an access-denied message. Unauthenticated callers are denied any menu item with `min_security_level > 0`.
- The starter main menu includes a `S` Sysop submenu entry gated with `min_security_level = 255`, accessible only to sysop-level callers.

The local sysop CLI is controlled by local machine access, config path/database
access, and the Unix control socket boundary. It is not a remote caller privilege
system.

## Managing User Levels

List users:

```bash
oxidebbs-server users list
```

Show one user:

```bash
oxidebbs-server users show <alias-or-id>
```

Create a user at a specific level:

```bash
oxidebbs-server users add --alias test --password "change-this" --level 10
```

Change an existing user's level:

```bash
oxidebbs-server users set-level <alias-or-id> 50
```

Promote a sysop:

```bash
oxidebbs-server users promote-sysop <alias-or-id>
```

Demote a sysop and lower the account back to a normal caller level:

```bash
oxidebbs-server users demote-sysop <alias-or-id>
oxidebbs-server users set-level <alias-or-id> 10
```

Disable an account instead of using level `0` as a lockout:

```bash
oxidebbs-server users disable <alias-or-id>
```

## Managing Message-Area Levels

List message areas and their gates:

```bash
oxidebbs-server messages areas list
```

Create an area with explicit read/post levels:

```bash
oxidebbs-server messages areas add private \
  --name "Private" \
  --read-level 50 \
  --post-level 50
```

Change an existing area's gates:

```bash
oxidebbs-server messages areas set-level private --read 50 --post 50
```

## Suggested Local Policy

This table is a convention, not built-in behavior:

| Level | Suggested use |
| --- | --- |
| `0` | Read-only or very limited caller policy |
| `10` | Normal new caller |
| `50` | Trusted caller or private-area access |
| `100` | Staff or elevated caller workflows |
| `255` | Sysop account paired with `is_sysop = true` |

Use account status for lockouts and removals. Security levels should describe
what an active user may access, not whether the user can log in at all.

## Source Of Truth

The user-level range is enforced by the core authentication flow and DecentDB
schema constraints. The default new-user level lives in
`config/oxidebbs.example.toml` and generated setup configs. Message-area gates
live in the `message_areas` table as `read_security_level` and
`post_security_level`.
