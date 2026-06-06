# FTN / OxideNet Networking Design Notes

## Product intent

OxideBBS is adding FTN-style shared message-network support through the v1.2
release plan.

Possible network branding: **OxideNet**.

## Design principle

Do not hard-code FidoNet only. Model this as FTN-style networks.

Examples:

- FidoNet
- fsxNet
- RetroNet-style networks
- OxideNet

## Current foundation

The shared network foundation now lives in `oxidebbs-network` and the
schema-backed runtime state lives in DecentDB `network_*` tables.

Implemented foundation concepts:

- network profiles and links
- zone/net/node/point FTN-style addresses
- echomail area mappings
- netmail and network message envelopes
- seen-by/path metadata tables
- duplicate detection keys and logs
- packet import/export boundaries
- queue state enums
- poll logs and schedules

## Suggested milestones

### Foundation

- Internal network address model
- Area mapping
- Echomail-ready schema
- Netmail-ready schema

### Import/export

- FTN packet reader
- FTN packet writer
- Duplicate detection
- Toss/scanner workflow

### Transport

- BinkP polling for FTN/FidoNet mail exchange
- Archive bundling
- Inbound/outbound queues

## Keep separate

Local message base support should work before FTN networking is implemented.
