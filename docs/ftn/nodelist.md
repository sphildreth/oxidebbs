# FTN Nodelists

OxideBBS stores imported FTN nodelist rows in DecentDB `network_nodelist`
records keyed by network profile and FTN address.

Import a full nodelist:

```bash
oxidebbs-server net nodelist import NODELIST.123 --network fidonet
```

Apply a plain text `NODEDIFF.xxx` to a full base nodelist and import the
resulting entries:

```bash
oxidebbs-server net nodelist apply-diff NODEDIFF.130 --base NODELIST.123 --network fidonet
```

If only one network profile exists, `--network` may be omitted. When multiple
profiles exist, pass the profile key explicitly.

List or look up entries:

```bash
oxidebbs-server net nodelist list --network fidonet --limit 100
oxidebbs-server net nodelist lookup 1:105/42 --network fidonet
oxidebbs-server net nodelist lookup 1:105/42.7 --network fidonet
```

Parser scope:

- skips blank lines and semicolon comment lines
- uses `Zone` and `Host` rows as address context
- imports normal comma-prefixed node rows
- imports `Pvt`, `Hold`, `Down`, `Hub`, and `Boss` node rows
- imports `Point` rows under the most recent boss node
- normalizes underscores in parsed names to spaces

Differential update scope:

- supports the FTS-style count-command format: `A<count>`, `C<count>`, and
  `D<count>`
- requires the first line of the diff to match the first line of the supplied
  base nodelist
- rejects unsupported commands, invalid counts, exhausted add data, and
  copy/delete ranges beyond the base nodelist
- parses the applied output and atomically replaces the stored DecentDB rows for
  the selected network profile

Administrative `Zone`, `Region`, and `Host` rows are not stored as nodelist
entries because the current schema stores concrete nonzero node addresses.

Current limitations:

- compressed nodelist and nodediff archives must be extracted before import or
  apply-diff
- nodediff CRC validation is not implemented yet
- flags, phone numbers, sysop names, and location fields are preserved only in
  the raw entry text today
- netmail routing and AreaFix do not consume the nodelist yet
