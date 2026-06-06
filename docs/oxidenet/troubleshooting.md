# Troubleshooting

`net oxidenet status` is the first check. It shows whether the profile exists,
how many applications are pending, and how many nodes are registered.

Common issues:

| Symptom | Check |
| --- | --- |
| `OxideNet profile is not installed` | Run `net oxidenet install-hub` on the hub or import a config package on a member. |
| Poll rejected as suspended | Run `net oxidenet nodes activate <node>` on the hub. |
| Package import fails validation | Verify all five package files are present and that credentials match `oxidenet.toml`. |
| No areas after import | Run `net areas list --network oxidenet`; re-import is idempotent for missing areas. |
| Token limit reached | Revoke unused invite-token credentials before issuing another token. |

Use the TUI OxideNet screen for a quick view of pending applications,
suspended nodes, packet queues, quarantine counts, subscriptions, and poll logs.
