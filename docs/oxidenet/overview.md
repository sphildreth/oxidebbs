# OxideNet

OxideNet is the first-party FTN-style network profile for OxideBBS. It uses the
shared network schema, DecentDB registry tables, BinkP polling, and the
`oxidebbs-oxidenet` workflow service.

Implemented v1.2 workflows:

- OxideNet defaults for zone `42`, primary hub `42:1/1`, backup hub `42:1/2`,
  member range `42:1/100-899`, test range `42:1/900+`, and default areas.
- BBS-native application records with validation, status lookup, admin review,
  approval, hold, reject, and request-info states.
- Automatic member address assignment, session credential generation, join
  token issue/revoke, node suspension/reactivation, and password rotation.
- Config package generation/import using `oxidenet.toml`, `areas.toml`,
  `nodelist.toml`, `credentials.toml`, and `manifest.toml`.
- Hub/member filesystem simulation through package export/import.
- OxideNet poll enforcement through the BinkP poller, including suspended-node
  rejection and poll timestamp recording.
- Local sysop TUI screens for dashboard, applications, nodes, queues,
  subscriptions, poll logs, nodelist generation, and config package operations.

Common commands:

```bash
oxidebbs-server net oxidenet install-hub --host blackboard.example.net
oxidebbs-server net oxidenet apply --board-name "Retro BBS" --sysop-alias Sysop \
  --contact-email sysop@example.net --host retro.example.net \
  --description "ANSI and echomail" --reason "Join OxideNet"
oxidebbs-server net oxidenet applications list
oxidebbs-server net oxidenet applications approve <application-id> --package-dir ./package
oxidebbs-server net oxidenet package import ./package
oxidebbs-server net poll oxidenet-hub
oxidebbs-server net oxidenet nodelist --output ./public/oxidenet.nodelist
```
