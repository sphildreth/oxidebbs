# OxideNet Config Package

OxideNet config packages are planned as a local onboarding bundle for approved
member sysops.

The current `oxidebbs-oxidenet` crate models and validates these package files:

| File | Purpose |
| --- | --- |
| `oxidenet.toml` | Network key/name, assigned address, hub address, local board metadata, hub endpoint, session password, and accepted policy metadata. |
| `areas.toml` | Default area tags, local area keys, display names, and subscription defaults. |
| `nodelist.toml` | Known OxideNet nodes with address, board, sysop, host, BinkP port, and status. |
| `credentials.toml` | Member address, hub address, and the one-time plaintext session password. |

Validation currently checks:

- network key is `oxidenet`
- assigned address is an assignable member or test/lab address
- hub address is a primary, backup, or infrastructure address
- hub host, BinkP port, poll interval, local board name, sysop alias, policy
  version, and policy acceptance timestamp are present
- credential address, hub address, and password match the network section
- area tags are uppercase ASCII using letters, digits, `.`, `_`, or `-`
- at least one area and one nodelist node are present

The hub must store only credential hashes after package generation. The
plaintext session password belongs only in the delivered config package and is
not persisted in hub registry state.

Generation, import, token lifecycle, and package archival are still planned.
