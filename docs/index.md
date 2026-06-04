---
layout: home

hero:
  name: OxideBBS
  text: A modern BBS server implementation
  tagline: Telnet callers, ANSI/CP437 screens, DecentDB persistence, and isolated DOS doors.
  actions:
    - theme: brand
      text: Get Started
      link: /project/getting-started
    - theme: alt
      text: Architecture
      link: /project/architecture

features:
  - title: Telnet-first
    details: Classic telnet callers are supported now. Serial, modem, and caller file-transfer work is tracked in the v1.2 release plan.
  - title: ANSI/CP437-native
    details: Caller output stays byte-oriented, with 40-column and 80-column profiles as first-class layouts.
  - title: DecentDB-backed
    details: OxideBBS uses DecentDB as its only system database through native Rust bindings.
  - title: Door-ready
    details: Door execution is isolated from core session logic, with drop files and DOS runtime runners.
---
