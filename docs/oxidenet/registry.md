# OxideNet Registry

OxideNet registry storage is present in DecentDB schema `8`. It is the local
data-model foundation for future BBS-native application review, address
assignment, node lifecycle tracking, and credential rotation.

The registry is not a live public network workflow yet. It stores operator data
and gives backup/import paths a durable shape while application screens,
approval commands, package generation, hub polling, and suspension workflows are
completed.

## Tables

- `network_applications` stores application metadata, lifecycle status, optional
  applicant/reviewer user references, policy acceptance, review notes, and the
  assigned address when one has been chosen.
- `network_nodes` stores assigned node records, address parts, hub address,
  contact/routing metadata, lifecycle timestamps, poll timestamps, and nodelist
  flags.
- `network_credentials` stores per-node credential hashes for BinkP sessions and
  invite tokens. Plaintext secrets are not stored.

## Backup and Restore

`oxidebbs-server db export --format json` includes:

- `oxidenet_applications`
- `oxidenet_nodes`
- `oxidenet_credentials`

`db import --format json` restores those sections after shared `network_*`
tables and before runtime sessions, doors, and audit events. Restore validation
checks duplicate application IDs, duplicate node addresses, missing node
references, known lifecycle labels, valid port ranges, and nonblank credential
hashes.

## Current Boundaries

- Registry rows can be inserted and queried through `oxidebbs-db` repository
  APIs.
- Address parsing/classification and config-package validation live in
  `oxidebbs-oxidenet`.
- BBS-native application submission, sysop review screens, token issuing,
  config-package generation/import, hub polling, and suspension enforcement are
  still future OxideNet runtime work.
