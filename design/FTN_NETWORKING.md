# FTN / OxideNet Networking Design Notes

## Product intent

OxideBBS should eventually support FTN-style shared message networks.

Possible network branding: **OxideNet**.

## Design principle

Do not hard-code FidoNet only. Model this as FTN-style networks.

Examples:

- FidoNet
- fsxNet
- RetroNet-style networks
- Future OxideNet

## Future data concepts

- Network
- Zone
- Net
- Node
- Point
- Network address
- Echomail area
- Netmail message
- Seen-by/path metadata
- Duplicate hash
- Packet import job
- Packet export job
- Poll schedule

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
