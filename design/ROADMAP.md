# OxideBBS Roadmap

## Milestone 0 — Repo foundation

- [x] Workspace compiles
- [x] Documentation baseline
- [x] Example config
- [x] ADRs established
- [x] Basic CI checks

## Milestone 1 — Terminal foundation

- [x] ANSI asset loader
- [x] CP437 conversion helper
- [x] Screen renderer
- [ ] Line input
- [x] Pager
- [ ] Welcome/logoff screens

## Milestone 2 — Telnet sessions

- [ ] Telnet listener
- [x] Telnet negotiation parser
- [x] Transport trait
- [x] Session task lifecycle
- [ ] Node assignment
- [ ] Idle timeout
- [ ] Clean disconnect

## Milestone 3 — Users and menus

- [x] New user flow
- [x] Login flow
- [x] Password hashing
- [x] User stats
- [x] Security level
- [x] Configurable menus
- [x] Hotkey routing

## Milestone 4 — DecentDB persistence

- [x] Database open/init
- [x] Schema/version tracking
- [x] User repository
- [x] Message repository
- [x] Door repository
- [x] Audit event repository
- [ ] DbWriter service

## Milestone 5 — Local messages

- [x] Message areas
- [x] Read message
- [x] Post message
- [x] Reply to message
- [x] Private mail foundation
- [x] Moderation primitives

## Milestone 6 — Doors

- [x] Door definition TOML
- [x] Node runtime directories
- [x] DOOR.SYS writer
- [x] DORINFO1.DEF writer
- [x] Door test command
- [x] DOSBox runner
- [x] Timeout cleanup
- [x] Door run logging

## Milestone 7 — Sysop tools

- [x] CLI user management
- [x] CLI node view
- [x] CLI door test
- [x] CLI config check
- [x] Local Ratatui console prototype

## Milestone 8 — FTN/OxideNet foundation

- [x] Network address model
- [x] Echomail-ready schema
- [x] Netmail-ready schema
- [x] Area mapping
- [x] Duplicate detection design
- [x] Packet import/export design

## Future

- [ ] Physical serial/modem transport
- [ ] BinkP polling
- [ ] Full FTN tosser/scanner
- [ ] OxideNet network support
- [ ] File transfer support, if still desired
