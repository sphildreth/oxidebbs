# OxideBBS Roadmap

## Milestone 0 — Repo foundation

- [ ] Workspace compiles
- [ ] Documentation baseline
- [ ] Example config
- [ ] ADRs established
- [ ] Basic CI checks

## Milestone 1 — Terminal foundation

- [ ] ANSI asset loader
- [ ] CP437 conversion helper
- [ ] Screen renderer
- [ ] Line input
- [ ] Pager
- [ ] Welcome/logoff screens

## Milestone 2 — Telnet sessions

- [ ] Telnet listener
- [ ] Telnet negotiation parser
- [ ] Transport trait
- [ ] Session task lifecycle
- [ ] Node assignment
- [ ] Idle timeout
- [ ] Clean disconnect

## Milestone 3 — Users and menus

- [ ] New user flow
- [ ] Login flow
- [ ] Password hashing
- [ ] User stats
- [ ] Security level
- [ ] Configurable menus
- [ ] Hotkey routing

## Milestone 4 — DecentDB persistence

- [ ] Database open/init
- [ ] Schema/version tracking
- [ ] User repository
- [ ] Message repository
- [ ] Door repository
- [ ] Audit event repository
- [ ] DbWriter service

## Milestone 5 — Local messages

- [ ] Message areas
- [ ] Read message
- [ ] Post message
- [ ] Reply to message
- [ ] Private mail foundation
- [ ] Moderation primitives

## Milestone 6 — Doors

- [ ] Door definition TOML
- [ ] Node runtime directories
- [ ] DOOR.SYS writer
- [ ] DORINFO1.DEF writer
- [ ] Door test command
- [ ] DOSBox runner
- [ ] Timeout cleanup
- [ ] Door run logging

## Milestone 7 — Sysop tools

- [ ] CLI user management
- [ ] CLI node view
- [ ] CLI door test
- [ ] CLI config check
- [ ] Local Ratatui console prototype

## Milestone 8 — FTN/OxideNet foundation

- [ ] Network address model
- [ ] Echomail-ready schema
- [ ] Netmail-ready schema
- [ ] Area mapping
- [ ] Duplicate detection design
- [ ] Packet import/export design

## Future

- [ ] Physical serial/modem transport
- [ ] BinkP polling
- [ ] Full FTN tosser/scanner
- [ ] OxideNet network support
- [ ] File transfer support, if still desired
