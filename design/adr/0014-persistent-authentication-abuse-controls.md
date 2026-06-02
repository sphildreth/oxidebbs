# ADR 0014: Add Persistent Authentication Abuse Controls

## Status

Accepted

## Context

The telnet login flow currently retries indefinitely on failed credentials.
Argon2id slows each password guess, but there is no per-IP or per-alias
throttling, no temporary lockout, and no persistence across server restarts.

The login flow also takes measurably different paths for missing aliases and
existing aliases with wrong passwords. A missing alias returns without running
Argon2 verification, while a wrong password for an existing alias verifies the
stored PHC string. That creates a timing oracle for alias enumeration.

## Decision

Add persistent authentication abuse controls before v1 release.

The implementation MUST use DecentDB. It MUST NOT introduce Redis, SQLite,
Postgres, or any external rate-limit store.

### Persistent Attempt Counters

Add a DecentDB table named `auth_attempts` with one row per scope:

- `scope`: either `ip` or `alias`
- `scope_key`: remote IP string for `ip`, normalized lowercase alias for
  `alias`
- `failed_count`
- `first_failed_at`
- `last_failed_at`
- `locked_until`

The pair `(scope, scope_key)` MUST be unique.

### Lockout Policy

The lockout policy is fixed for v1:

- five failed attempts
- within a ten minute window
- locks the matching scope for fifteen minutes

Both scopes are checked before password verification:

- remote IP scope
- normalized alias scope

If either scope is locked, the login flow MUST return:

```text
Too many login attempts. Try again later.
```

The message MUST NOT say whether the IP scope or alias scope caused the
lockout.

On failed credentials, both scopes MUST be incremented. On successful login,
both scopes MUST be reset.

### Timing Equalization

Alias misses MUST run one Argon2 verify operation against a process-static dummy
PHC string before returning failure.

Unparseable stored password hashes MUST also run the dummy verify operation and
then fail closed. The failure MUST emit an audit event type named
`password_hash_parse_failure`.

The visible failed-login response for missing alias and wrong password MUST
remain:

```text
Invalid alias or password. Please try again.
```

### Argon2 Parameters

Argon2id remains the password hashing algorithm.

The default parameters are explicit:

- memory cost: 19456 KiB
- iterations: 2
- parallelism: 1

Expose these values in config under an authentication section before changing
the defaults. Agents MUST NOT choose different values without updating this ADR.

### New User Security Level

Expose the new-user starting security level in config. The default remains
`10`. Generated config and example config MUST show this default.

## Consequences

- Online guessing is bounded by persistent per-IP and per-alias controls.
- Restarting the server does not reset active lockouts.
- Alias enumeration by timing is materially reduced.
- Invalid stored hashes are visible in audit logs instead of silently looking
  like wrong passwords.
- Authentication config changes require schema and config documentation updates.

## Rejected Options

- In-memory-only rate limiting: it resets on restart and does not satisfy the
  review concern.
- Per-connection attempt limiting only: attackers can reconnect.
- Returning different messages for missing alias, wrong password, and lockout
  scope: that leaks account or deployment state.
