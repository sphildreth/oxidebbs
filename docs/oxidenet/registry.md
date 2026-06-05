# OxideNet Registry

DecentDB schema `8` stores OxideNet registry state.

Tables:

- `network_applications`: application metadata, lifecycle status, policy
  acceptance, review notes, reviewer, and assigned address.
- `network_nodes`: assigned node records, routing metadata, lifecycle status,
  poll timestamps, suspension state, and nodelist flags.
- `network_credentials`: per-node hashes for BinkP session passwords and invite
  tokens.

Lifecycle commands:

```bash
oxidebbs-server net oxidenet applications list
oxidebbs-server net oxidenet applications show <application-id>
oxidebbs-server net oxidenet applications approve <application-id>
oxidebbs-server net oxidenet applications reject <application-id>
oxidebbs-server net oxidenet applications request-info <application-id>
oxidebbs-server net oxidenet applications hold <application-id>

oxidebbs-server net oxidenet nodes list
oxidebbs-server net oxidenet nodes suspend 42:1/100
oxidebbs-server net oxidenet nodes activate 42:1/100
oxidebbs-server net oxidenet nodes rotate-password 42:1/100

oxidebbs-server net oxidenet tokens issue 42:1/100
oxidebbs-server net oxidenet tokens revoke <credential-id>
```

Suspended nodes are rejected by the OxideNet poll path before BinkP exchange.
Poll attempts update the node registry and the shared network poll log.
