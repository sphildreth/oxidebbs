# OxideNet Config Package

Approved member boards receive a local config package. The hub writes only
credential hashes to DecentDB; plaintext secrets are shown once and written only
to the package delivered to the member sysop.

Package files:

| File | Purpose |
| --- | --- |
| `oxidenet.toml` | Network key/name, assigned address, hub address, board metadata, hub endpoint, session password, and accepted policy metadata. |
| `areas.toml` | Default area tags, local area keys, display names, and subscription defaults. |
| `nodelist.toml` | Known OxideNet nodes with address, board, sysop, host, BinkP port, and status. |
| `credentials.toml` | Member address, hub address, and plaintext session password. |
| `manifest.toml` | Package generation timestamp and token hash. |

Generate during approval:

```bash
oxidebbs-server net oxidenet applications approve <application-id> \
  --package-dir ./oxidenet-package
```

Generate again after password rotation:

```bash
oxidebbs-server net oxidenet nodes rotate-password 42:1/100
oxidebbs-server net oxidenet package generate 42:1/100 \
  --session-password <plaintext-from-rotation> \
  --output ./oxidenet-package
```

Import on a member board:

```bash
oxidebbs-server net oxidenet package import ./oxidenet-package
```

Import creates the `oxidenet` network profile, the `oxidenet-hub` BinkP link,
local echomail areas, and network-area mappings when missing.
