# DOOR_GAME_RESOURCES.md

# Free / Legally Usable BBS Door Game Resources for OxideBBS

## Purpose

This document collects resources, workflows, and cataloging recommendations for finding BBS door games that are usable without requiring a monetary payment.

For this document, “free” means one of the following:

1. **Open source** — source is available under a clear open-source license.
2. **Freeware** — the author or maintainer explicitly released the door as free to use.
3. **Donationware** — payment is optional and the author/maintainer allows use without payment.
4. **Released keys included** — the door is still registered/keyed software, but the author, maintainer, or trusted archive provides a legitimate free key.
5. **Author permission / documented release** — there is credible evidence that the author or rights holder allowed free use.
6. **Archive candidate requiring verification** — the file exists in a historical archive, but the actual license/readme must be inspected before use.

This document is **not legal advice**. It is a practical sysop/developer guide for building a clean door-game test set for OxideBBS.

## Recommended policy for OxideBBS

OxideBBS should avoid bundling door games directly unless their license clearly permits redistribution.

Instead, OxideBBS should provide:

- A curated resource list.
- Door configuration examples.
- A door catalog format.
- A verification workflow.
- Setup guides for known legally usable/freeware/donationware doors.
- A strict no-cracks/no-keygens policy.

Recommended project stance:

```text
OxideBBS does not distribute copyrighted third-party door games unless the
license clearly allows redistribution. Sysops are responsible for installing
their own doors. OxideBBS may provide configuration templates for legally
freeware, open-source, donationware, or author-released doors.
```

## Legal/use-status categories

Use these categories when cataloging doors.

| Code | Status | Meaning | Safe for OxideBBS docs/examples? |
|---|---|---|---|
| A | Open source | Clear source license, such as GPL, MIT, Apache, BSD, etc. | Yes, cite license |
| B | Freeware | Explicit freeware statement by author/maintainer/archive | Usually yes |
| C | Donationware | Payment optional; use allowed without payment | Usually yes |
| D | Free registered/keyed | Free registration key provided by author/maintainer/trusted archive | Usually yes, verify provenance |
| E | Shareware playable | Usable in limited mode without payment | Mention only with limitations |
| F | Historical/archive candidate | Exists in archive, license unclear | Do not recommend until verified |
| X | Avoid | Crack/keygen/pirated/unclear redistribution | No |

For OxideBBS testing and documentation, prefer **A–D**.

## High-confidence starting points

These are the best starting points for building an initial clean test catalog.

---

## 1. Synchronet BBS Door Distribution Sites page

**Use for:** finding major door archives and known door distribution sites.

Synchronet’s wiki has a “BBS Door Distribution Sites” page that lists several important resources:

- BBS Archives and Development News site-rip
- bbsfiles.com site-rip
- DoorWare Distribution System
- Sunrise Door Software
- Sysops’ Corner
- Retrocomputing ArchivE

Source:

- https://wiki.synchro.net/resource%3Adoors

Why it matters:

This is a good hub page because Synchronet is one of the major modern BBS platforms still used by active sysops. Its door documentation and resource pages tend to point toward archives that real sysops use.

Caution:

The page is a directory of resources. It does **not** mean every linked archive is freeware. Use it as a map, not as legal proof.

Recommended OxideBBS usage:

```text
docs/doors/resources.md:
  "Start with Synchronet's door distribution resources to discover
   candidate doors, then inspect each archive for license/readme evidence."
```

---

## 2. DoorWare Distribution System / DoorGames.org freeware index

**Use for:** freeware door candidates, especially Bob Dalton doors with included key codes.

The DoorWare Distribution System freeware index is one of the most relevant sources for this project because it has a dedicated “Freeware BBS Files” section.

A particularly useful entry describes a Bob Dalton freeware DOS door archive containing 11 DOS ANSI/ASCII door games and their key codes.

Source:

- https://www.doorgames.org/indexes/freeware.htm

Important detail:

The index entry for the Bob Dalton archive says it includes freeware DOS door games and key codes. This makes it a strong early candidate for OxideBBS door-runner testing.

Candidate games listed in that archive include:

- GodFather of Crime
- Rise To Power
- Death by Trivia
- Adventure Door Game Toolkit
- CampTown Races
- Escape
- Grunt Fest
- Ship of the Line
- Task Force Broadside
- Way Freight
- BioGuide

Recommended legal status:

```text
Status: D - Free registered/keyed
Confidence: Medium-high
Evidence needed: inspect archive README/registration files and preserve notes
```

Recommended OxideBBS usage:

This should be one of the first collections to test because:

- It targets DOS ANSI/ASCII door-game behavior.
- It includes keys according to the archive index.
- It likely exercises classic drop-file behavior.
- It should be easier to justify than random abandonware.

Recommended catalog entry:

```toml
[[door_resource]]
name = "Bob Dalton Freeware DOS Door Games"
source_url = "https://www.doorgames.org/indexes/freeware.htm"
status = "freeware-with-keys"
confidence = "medium-high"
notes = "Index describes 11 FREEWARE DOS ANSI/ASCII door games with key codes."
verify = [
  "Download archive",
  "Inspect README/DOC/REGISTER/FREE-KEY files",
  "Record SHA256",
  "Record supported drop files",
  "Test in DOSEMU2 runner"
]
```

---

## 3. Sunrise Door Software collection

**Use for:** freeware/donationware door-game candidates.

Sunrise Door Software is one of the more promising sources because multiple references state that Sunrise’s BBS door products were released as Freeware/Donationware.

Sources:

- Google Groups post referencing Sunrise release status:
  - https://groups.google.com/g/alt.bbs.elebbs/c/GMDZmLFnyoI
- SynchroFans/Synchronet download page for an installed Sunrise door collection:
  - https://synchronetbbs.org/index.php/downloads/download/3-doors/30-bbsdoors

Important detail:

The SynchroFans download page states that Sunrise BBS door products have been released as Freeware/Donationware and provides a collection with the doors already extracted from their installers.

Recommended legal status:

```text
Status: B/C - Freeware/Donationware
Confidence: Medium-high
Evidence needed: keep source page notes and inspect bundled docs
```

Recommended OxideBBS usage:

This is another excellent early test set. Because the collection is described as already extracted from installers, it may be easier to stage for DOSEMU2 tests.

Recommended catalog entry:

```toml
[[door_resource]]
name = "Sunrise Door Collection"
source_url = "https://synchronetbbs.org/index.php/downloads/download/3-doors/30-bbsdoors"
status = "freeware-donationware"
confidence = "medium-high"
notes = "Download page says Sunrise BBS door products were released as Freeware/Donationware."
verify = [
  "Inspect docs inside collection",
  "Record individual door names",
  "Record supported drop files",
  "Test at least one door with DOSEMU2 runner"
]
```

---

## 4. Synchronet Vertrauen door file areas

**Use for:** practical sysop-tested downloads and door collections.

Synchronet’s Vertrauen BBS file areas include door archives and are relevant because Synchronet is widely used by modern BBS sysops.

Source:

- https://web.synchro.net/?dir=doors&page=002-files.xjs

Why it matters:

The file area has practical door archives. Some descriptions explicitly identify freeware collections or key-included collections.

Caution:

Treat each archive individually. A file being hosted in a BBS file area is not automatically a license grant.

Recommended OxideBBS usage:

Use as a discovery source and compare with DoorWare/Sunrise/source-code evidence.

---

## Useful but verify-before-use archives

The following resources are valuable for discovery but require careful per-archive license inspection.

---

## 5. BBS Archives / archives.thebbs.org mirrors

**Use for:** broad historical discovery.

The old BBS Archives material has been mirrored in several places, including Archeobits.

Source:

- https://mirrors.archeobits.com/bbs/archives.thebbs.org/index.html

Why it matters:

This is a large historical archive of BBS-related files. It may contain many doors, utilities, docs, and historical packages.

Caution:

This should be treated as a candidate source only. Many archives are historical shareware or abandoned files with unclear current licensing. Do not assume “archived” means “free.”

Recommended legal status:

```text
Default status: F - Historical/archive candidate
```

Recommended workflow:

1. Download candidate archive.
2. Extract to a quarantine/staging directory.
3. Inspect all text files:
   - `README`
   - `README.TXT`
   - `LICENSE`
   - `REGISTER`
   - `ORDER`
   - `VENDOR`
   - `.DOC`
   - `.NFO`
   - `FREEKEY`
   - `FREE-KEY.TXT`
4. Search for:
   - “freeware”
   - “public domain”
   - “donationware”
   - “released”
   - “registration”
   - “key”
   - “permission”
   - “copyright”
5. Record exact evidence in the OxideBBS catalog.
6. If evidence is unclear, mark as `license-unclear`.

---

## 6. RetroArchive / Night Owl CD-ROM mirrors

**Use for:** broad historical discovery.

RetroArchive hosts historical CD-ROM file listings, including Night Owl BBS door and utility areas.

Source:

- https://www.retroarchive.org/cdrom/nightowl-005/025A/index.html

Why it matters:

The Night Owl CD-ROMs are historically relevant and include many BBS door and utility archives.

Caution:

Like other historical mirrors, this is not automatically a freeware source. It is a discovery source.

Recommended legal status:

```text
Default status: F - Historical/archive candidate
```

Recommended OxideBBS usage:

Use this to identify old door names, versions, authors, and candidate files. Only move a door into a recommended catalog after license evidence is found.

---

## 7. BBSFiles mirrors

**Use for:** some doors with included “FREE COPY” keys, but handle carefully.

Some BBSFiles mirror pages state that “FREE COPY” keys are included in archives where applicable and that those doors are fully functional with the provided key.

Example source:

- https://www.synchro.net/files/bbsfiles.com/ROBERTCH/DiSoft.htm

However, other BBSFiles license pages state that some software is not public domain and not free, and redistribution conditions may apply.

Example source:

- https://web.synchro.net/files/bbsfiles.com/CYBERSOF/license.html

Why it matters:

BBSFiles content can be useful, but the legal status varies by author/product. This source should not be blanket-approved.

Recommended legal status:

```text
Default status: F - Historical/archive candidate
Possible status: D - Free registered/keyed, only where evidence supports it
```

Recommended OxideBBS policy:

Do not treat `bbsfiles.com` or its mirrors as a single license category. Catalog every door separately.

Example note:

```toml
[[door_resource]]
name = "DiSoft Doors via BBSFiles mirror"
source_url = "https://www.synchro.net/files/bbsfiles.com/ROBERTCH/DiSoft.htm"
status = "free-copy-keys-claimed"
confidence = "medium"
notes = "Page says FREE COPY keys are included where applicable. Verify each archive individually."
```

---

## Reference and research resources

These resources are more useful for context, compatibility, and historical identification than for direct “free door” downloads.

---

## 8. Break Into Chat

**Use for:** door-game history, author identification, screenshots, names, and variants.

Source:

- https://breakintochat.com/
- https://breakintochat.com/wiki/BBS_door_game
- https://breakintochat.com/wiki/BBS_door

Why it matters:

Break Into Chat is a valuable historical/reference wiki for BBS and door-game culture. It can help identify:

- Original authors
- Game names and variants
- Historical context
- Screenshots
- Related projects
- Download/source leads

Caution:

A historical wiki entry is not automatically a license grant. Use it for research and attribution clues.

Recommended OxideBBS usage:

Use Break Into Chat to enrich door catalog entries:

```toml
history_url = "https://breakintochat.com/wiki/..."
author = "..."
notes = "Historical background from Break Into Chat; license verified separately."
```

---

## 9. Synchronet door installation docs

**Use for:** understanding how modern BBS software handles doors.

Source:

- https://wiki.synchro.net/howto%3Adoor%3Aindex
- https://wiki.synchro.net/config%3Aexternal_programs

Why it matters:

Synchronet’s door docs are useful for understanding practical door installation concepts:

- External programs
- Drop files
- Per-door config
- Per-node behavior
- Command lines
- Door execution expectations

Recommended OxideBBS usage:

Use this as a compatibility model, not as something to copy literally. OxideBBS should have its own Rust/DecentDB-oriented design, but Synchronet’s docs help define what sysops expect.

---

## Door runners, servers, and compatibility references

These are useful for OxideBBS implementation research.

---

## 10. DoorNode

**Use for:** implementation inspiration for legacy DOS door launching.

Source:

- https://github.com/dinchak/doornode

DoorNode is a Node.js application for launching BBS door games on modern operating systems. It was designed for MajorBBS/WorldGroup via RLogin but is still relevant because it shows how modern systems can wrap old DOS doors.

Recommended OxideBBS usage:

Study ideas, not code dependencies:

- emulator configuration
- Door launch lifecycle
- Runtime directory management
- Debugging approach
- Multi-node considerations

---

## 11. GameSrv

**Use for:** implementation inspiration for a door-game server model.

Sources:

- https://github.com/rickparrish/GameSrv
GameSrv is a BBS door game server for Windows.

Recommended OxideBBS usage:

Use as proof that modern front-end/telnet plus isolated door execution is a practical approach.

---

## 12. ENiGMA½ local doors docs

**Use for:** modern BBS door integration patterns.

Source:

- https://nuskooler.github.io/enigma-bbs/modding/local-doors.html

Why it matters:

ENiGMA½ supports local doors and remote door server modules. Its docs are useful for comparing modern BBS expectations around local door execution.

Recommended OxideBBS usage:

Use as background for:

- Local doors
- Door server integrations
- Future BBSLink/DoorParty-style integrations

---

## Remote game services

Remote services are not local DOS doors, but they may be useful for future OxideBBS integrations.

---

## 13. BBSLink

**Use for:** future remote-door integration target.

Source:

- https://bbslink.net/

BBSLink describes itself as a free InterBBS games server that allows sysops to add multiplayer door games to their systems.

Why it matters:

BBSLink is not the same as running DOS doors locally, but it is relevant because many modern BBSes integrate with remote door/game services.

Recommended OxideBBS usage:

Do not make BBSLink part of v1. Treat it as a future “remote door provider” integration.

Possible future abstraction:

```text
DoorProvider
  LocalDosDoorProvider
  LocalNativeDoorProvider
  RemoteBbsLinkProvider
  RemoteDoorPartyProvider
```

---

## Open-source / source-available door and door-adjacent projects

These can be useful for testing, inspiration, or future native door support. They are not all DOS doors.

---

## 14. Usurper source/ports

**Use for:** source-available door-game research.

Source:

- https://github.com/rickparrish/Usurper

The repository describes 32-bit and 64-bit ports of the Usurper BBS doorgame.

Recommended OxideBBS usage:

Investigate license and build/runtime details before recommending as an OxideBBS test door. This may be more useful as a modern native/ported door candidate than as a DOSEMU2 v1 test.

---

## 15. Dominion

**Use for:** source-available BBS door project reference.

Source:

- https://github.com/mostlygeek/dominion

The repository is a BBS door game and includes ANSI assets.

Recommended OxideBBS usage:

Good source for understanding modern door-game source layout and ANSI asset handling. Verify license before recommending.

---

## 16. FrotzDoor

**Use for:** open-source/native door inspiration.

Source:

- https://github.com/fstltna/FrotzDoor

FrotzDoor is a Frotz interpreter hardened for use as a BBS door. The repository indicates GPL-2.0 licensing.

Recommended OxideBBS usage:

This is interesting because it is door-adjacent and open source. It may be a good future compatibility candidate for native/Linux door support, depending on how it handles terminal I/O and BBS integration.

---

## 17. Crazy Eights Slots

**Use for:** simple source-available ANSI BBS door reference.

Source:

- https://github.com/DRPanther/Crazy_Eights_Slots

The project describes itself as an ANSI BBS door game and includes C source/build materials.

Recommended OxideBBS usage:

Potentially useful as a small source-available test/reference door. Verify license before using in documentation or test fixtures.

---

## 18. Legend of the Green Dragon

**Use for:** BBS-door-inspired open-source web game reference.

Sources:

- https://github.com/stephenKise/Legend-of-the-Green-Dragon
- https://github.com/lotgd/core

Legend of the Green Dragon is an homage/remake inspired by the classic BBS door game Legend of the Red Dragon, but it is not a drop-in DOS door for OxideBBS v1.

Recommended OxideBBS usage:

Do not treat this as a DOS door test. It is useful as historical/cultural inspiration and possibly for thinking about future native/networked games.

---

## Development libraries and door-making references

These are useful if OxideBBS later encourages native modern doors.

---

## 19. OpenDoors

**Use for:** understanding classic door development libraries.

Source:

- https://synchronetbbs.org/index.php/downloads/category/3-doors?lms=7&start=90

The SynchroFans page includes OpenDoors 6.20, described as a C/C++ door programming toolkit.

Recommended OxideBBS usage:

Study expected APIs and drop-file support. Do not necessarily adopt it.

---

## 20. GoDoors

**Use for:** modern native door development inspiration.

Source:

- https://github.com/robbiew/godoors

GoDoors is described as a helper library for creating Linux-based door applications that use stdin/stdout when connected over terminal programs.

Recommended OxideBBS usage:

Useful as inspiration for a future `oxide-door-sdk` or native Rust door API.

---

## OxideBBS door catalog recommendation

OxideBBS should maintain a catalog file for known/tested doors.

Suggested path:

```text
docs/doors/DOOR_CATALOG.md
```

or machine-readable:

```text
doors/catalog.toml
```

Recommended fields:

```toml
[[door]]
key = "death-by-trivia"
name = "Death by Trivia"
author = "Bob Dalton"
type = "dos"
status = "freeware-with-key"
confidence = "medium-high"

source_name = "DoorWare Distribution System"
source_url = "https://www.doorgames.org/indexes/freeware.htm"
archive_name = "DARK019R.ZIP"
archive_sha256 = ""

license_evidence = '''
DoorWare freeware index describes the Bob Dalton archive as freeware DOS
ANSI/ASCII door games with included key codes. Verify exact archive contents.
'''

redistribution_ok = "unknown"
use_ok = "likely"
key_included = true
key_source = "archive"

supported_drop_files = ["DORINFO1.DEF", "DOOR.SYS"]
preferred_drop_file = "DORINFO1.DEF"

runner = "dosemu"
tested_with_oxidebbs = false
tested_date = ""
notes = "Candidate for early OxideBBS door-runner testing."
```

## Verification workflow

Every candidate door should go through this workflow before being recommended.

### Step 1: Download to staging

Use a staging directory, not the live `doors/` directory.

```text
staging/doors/incoming/
```

### Step 2: Record source

Record:

- Source URL
- Download date
- Archive filename
- Archive size
- SHA256 hash
- Mirror/source name

### Step 3: Extract safely

Extract into a unique directory:

```text
staging/doors/extracted/<archive-name>/
```

Do not run anything yet.

### Step 4: Inspect text evidence

Look for:

```text
README
README.TXT
LICENSE
COPYING
REGISTER
REGISTRATION
ORDER
ORDER.FRM
VENDOR
FILE_ID.DIZ
*.DOC
*.TXT
*.NFO
FREEKEY
FREE-KEY.TXT
KEY.TXT
```

Search for terms:

```text
freeware
public domain
donationware
registration
registered
key
serial
copyright
permission
license
distribution
shareware
```

### Step 5: Assign status

Use the A/B/C/D/E/F/X status categories.

If in doubt, use `F - historical/archive candidate`.

### Step 6: Identify drop-file support

Inspect docs/config for supported formats:

- `DOOR.SYS`
- `DORINFO1.DEF`
- `CHAIN.TXT`
- `DOORFILE.SR`
- `PCBOARD.SYS`
- `CALLINFO.BBS`

### Step 7: Dry-run with generated drop file

Use OxideBBS tooling:

```bash
oxidebbs doors dropfile <door-key> --user sysop --node 1 --format dorinfo1.def
oxidebbs doors test <door-key> --user sysop --dry-run
```

### Step 8: Runtime test

Test with:

- DOSEMU2 runner
- One local node
- Clean disconnect
- Time-limit enforcement
- Repeated start/exit cycles

### Step 9: Record result

Record:

- Works / does not work
- Runner used
- Drop file used
- Known issues
- Required config changes
- Whether multinode works
- Whether the door writes outside its directory

## Red flags

Avoid or quarantine doors with:

- Keygens
- Cracks
- Patchers
- “Registered to warez group”
- No author/license docs
- Unclear redistribution terms
- Installers that modify global paths unexpectedly
- Door expects direct COM port access only
- Door requires FOSSIL driver behavior that the v1 DOSEMU2 COM1 PTY bridge does
  not provide by itself
- Door writes to absolute paths outside its configured directory

## Recommended first OxideBBS test set

Start small and clean.

### Tier 1: likely best early candidates

1. **Bob Dalton freeware DOS doors**
   - Source: DoorWare freeware index
   - Why: described as freeware with key codes
   - Use: DOSEMU2/drop-file runner testing

2. **Sunrise Door collection**
   - Source: SynchroFans/Sunrise references
   - Why: described as Freeware/Donationware
   - Use: broader DOS door compatibility testing

3. **Open-source/source-available modern doors**
   - Examples: FrotzDoor, Crazy Eights Slots, Usurper ports
   - Why: source available, easier to debug
   - Use: future native/stdin-stdout compatibility

### Tier 2: discovery candidates

4. **Synchronet Vertrauen file area**
   - Use: find sysop-used archives
   - Verify: per archive

5. **BBS Archives mirrors**
   - Use: historical discovery
   - Verify: per archive

6. **RetroArchive/Night Owl mirrors**
   - Use: historical discovery
   - Verify: per archive

7. **BBSFiles mirrors**
   - Use: possible free-copy/key-included archives
   - Verify: very carefully per archive

## Suggested `DOOR_GAME_RESOURCES.md` maintenance rules

1. Do not call something freeware unless there is evidence.
2. Always cite the source URL.
3. Always record the archive hash after download.
4. Always preserve license/readme evidence.
5. Never recommend cracks/keygens.
6. Separate “usable by sysop” from “redistributable by OxideBBS.”
7. Prefer config templates over bundled third-party binaries.
8. Keep known-working door configs in the repo only if they do not include proprietary binaries.
9. Mark unverified resources clearly.
10. Revisit old links periodically because retro sites disappear.

## Example OxideBBS docs structure

```text
docs/
  doors/
    DOOR_GAME_RESOURCES.md
    DOOR_CATALOG.md
    DOOR_LEGAL_POLICY.md
    examples/
      bob-dalton/
        README.md
        door.example.toml
      sunrise/
        README.md
        door.example.toml
```

## Example door config template

```toml
[[doors]]
key = "death-by-trivia"
name = "Death by Trivia"
runner = "dosemu"
working_dir = "./doors/death-by-trivia"
command = "DBTRIVIA.EXE"
drop_file = "DORINFO1.DEF"
exclusive = false
time_limit_minutes = 30

[doors.legal]
status = "freeware-with-key"
source = "DoorWare Distribution System freeware index"
source_url = "https://www.doorgames.org/indexes/freeware.htm"
redistribution_ok = "unknown"
notes = "Sysop must obtain the archive separately and verify included key/license files."
```

## Summary recommendation

For OxideBBS, the clean approach is:

```text
Do not bundle old doors.
Do provide curated links.
Do provide config templates.
Do maintain a verification catalog.
Do start with Bob Dalton freeware doors and Sunrise freeware/donationware doors.
Do treat archive mirrors as discovery sources, not automatic license approval.
Do reject cracks/keygens outright.
```

This gives OxideBBS a responsible, contributor-friendly way to support classic BBS doors without turning the project into a legal gray-area archive.

## Source links

- Synchronet BBS Door Distribution Sites:
  - https://wiki.synchro.net/resource%3Adoors
- Synchronet door installation docs:
  - https://wiki.synchro.net/howto%3Adoor%3Aindex
- Synchronet external programs docs:
  - https://wiki.synchro.net/config%3Aexternal_programs
- DoorWare Freeware BBS Files:
  - https://www.doorgames.org/indexes/freeware.htm
- Sunrise Door Software newsletter/reference:
  - https://groups.google.com/g/alt.bbs.elebbs/c/GMDZmLFnyoI
- Sunrise Door Collection via SynchroFans:
  - https://synchronetbbs.org/index.php/downloads/download/3-doors/30-bbsdoors
- Synchronet Vertrauen doors file area:
  - https://web.synchro.net/?dir=doors&page=002-files.xjs
- BBS Archives mirror via Archeobits:
  - https://mirrors.archeobits.com/bbs/archives.thebbs.org/index.html
- RetroArchive Night Owl BBS doors/utils:
  - https://www.retroarchive.org/cdrom/nightowl-005/025A/index.html
- BBSFiles DiSoft mirror example:
  - https://www.synchro.net/files/bbsfiles.com/ROBERTCH/DiSoft.htm
- BBSFiles CyberSoft license example:
  - https://web.synchro.net/files/bbsfiles.com/CYBERSOF/license.html
- Break Into Chat:
  - https://breakintochat.com/
- Break Into Chat BBS door game page:
  - https://breakintochat.com/wiki/BBS_door_game
- Break Into Chat BBS door page:
  - https://breakintochat.com/wiki/BBS_door
- DoorNode:
  - https://github.com/dinchak/doornode
- GameSrv:
  - https://github.com/rickparrish/GameSrv
- ENiGMA½ local doors:
  - https://nuskooler.github.io/enigma-bbs/modding/local-doors.html
- BBSLink:
  - https://bbslink.net/
- Usurper ports/source:
  - https://github.com/rickparrish/Usurper
- Dominion:
  - https://github.com/mostlygeek/dominion
- FrotzDoor:
  - https://github.com/fstltna/FrotzDoor
- Crazy Eights Slots:
  - https://github.com/DRPanther/Crazy_Eights_Slots
- Legend of the Green Dragon:
  - https://github.com/stephenKise/Legend-of-the-Green-Dragon
- Legend of the Green Dragon core:
  - https://github.com/lotgd/core
- OpenDoors listing via SynchroFans:
  - https://synchronetbbs.org/index.php/downloads/category/3-doors?lms=7&start=90
- GoDoors:
  - https://github.com/robbiew/godoors
