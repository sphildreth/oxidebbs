# Sysop TUI Themes

The local sysop TUI supports selectable color themes with:

```bash
oxidebbs-server --config config/oxidebbs.toml sysop --theme oxide-classic
```

When `--theme` is omitted, OxideBBS uses `oxide-classic`.

## Available Themes

| Theme | Command | Character |
| --- | --- | --- |
| `oxide-classic` | `oxidebbs-server sysop --theme oxide-classic` | Charcoal console with oxide-orange headings, green online states, amber warnings, and red danger states. |
| `wildcat` | `oxidebbs-server sysop --theme wildcat` | Black and gray shell with bright cyan highlights, inspired by classic Wildcat-style sysop menus. |
| `telegard` | `oxidebbs-server sysop --theme telegard` | Dark blue sysop-console palette with bright blue focus states and muted blue-gray borders. |
| `vbbs` | `oxidebbs-server sysop --theme vbbs` | Dark green and teal palette inspired by VBBS-era utilitarian sysop screens. |
| `mystic` | `oxidebbs-server sysop --theme mystic` | Dark violet/purple palette inspired by Mystic BBS-style modern ANSI consoles. |
| `midnight` | `oxidebbs-server sysop --theme midnight` | Near-black and charcoal palette with muted gray accents and low-saturation status colors. |
| `high-contrast` | `oxidebbs-server sysop --theme high-contrast` | Accessibility-focused black, white, yellow, green, and red palette. |

`oxidebbs-server sysop --help` also lists the valid theme names.

## Menu Preview Examples

These examples are static documentation previews. They show the visual character
of each theme using the same kinds of labels, focus states, warnings, and node
status colors used by the local sysop TUI.

<div style="display:grid;grid-template-columns:repeat(auto-fit,minmax(260px,1fr));gap:1rem;margin:1rem 0">
  <div style="background:#141414;color:#dcdcdc;border:1px solid #646464;padding:0.8rem;font:0.9rem ui-monospace,SFMono-Regular,Menlo,Consolas,monospace">
    <div style="color:#ff8c00;font-weight:700">OxideBBS SYSOP</div>
    <div style="color:#a0a0a0">Theme: oxide-classic</div>
    <div style="margin-top:0.6rem"><span style="color:#ff8c00">></span> Nodes <span style="color:#00c800">03 Online</span></div>
    <div>  Users <span style="color:#ffb400">2 Pending</span></div>
    <div>  Doors <span style="color:#dc3232">1 Alert</span></div>
  </div>
  <div style="background:#0c0c0c;color:#d2d2d2;border:1px solid #60606e;padding:0.8rem;font:0.9rem ui-monospace,SFMono-Regular,Menlo,Consolas,monospace">
    <div style="color:#00b4f0;font-weight:700">WILDCAT SYSOP</div>
    <div style="color:#969696">Theme: wildcat</div>
    <div style="margin-top:0.6rem"><span style="color:#00b4f0">></span> Nodes <span style="color:#50ff78">03 Online</span></div>
    <div>  Users <span style="color:#ffc850">2 Pending</span></div>
    <div>  Doors <span style="color:#ff4040">1 Alert</span></div>
  </div>
  <div style="background:#081226;color:#d6dbe8;border:1px solid #687595;padding:0.8rem;font:0.9rem ui-monospace,SFMono-Regular,Menlo,Consolas,monospace">
    <div style="color:#5c9dff;font-weight:700">TELEGARD SYSOP</div>
    <div style="color:#9ca8c4">Theme: telegard</div>
    <div style="margin-top:0.6rem"><span style="color:#5c9dff">></span> Nodes <span style="color:#52d66b">03 Online</span></div>
    <div>  Users <span style="color:#ffb343">2 Pending</span></div>
    <div>  Doors <span style="color:#ff5c5a">1 Alert</span></div>
  </div>
  <div style="background:#10120f;color:#dcebdc;border:1px solid #6e8070;padding:0.8rem;font:0.9rem ui-monospace,SFMono-Regular,Menlo,Consolas,monospace">
    <div style="color:#00c6b2;font-weight:700">VBBS SYSOP</div>
    <div style="color:#a0b4a0">Theme: vbbs</div>
    <div style="margin-top:0.6rem"><span style="color:#00c6b2">></span> Nodes <span style="color:#58f08c">03 Online</span></div>
    <div>  Users <span style="color:#ffce54">2 Pending</span></div>
    <div>  Doors <span style="color:#ff6565">1 Alert</span></div>
  </div>
  <div style="background:#12121e;color:#d7d7f0;border:1px solid #727292;padding:0.8rem;font:0.9rem ui-monospace,SFMono-Regular,Menlo,Consolas,monospace">
    <div style="color:#9678ff;font-weight:700">MYSTIC SYSOP</div>
    <div style="color:#aaaacc">Theme: mystic</div>
    <div style="margin-top:0.6rem"><span style="color:#9678ff">></span> Nodes <span style="color:#8cf48c">03 Online</span></div>
    <div>  Users <span style="color:#ffd861">2 Pending</span></div>
    <div>  Doors <span style="color:#ff6c74">1 Alert</span></div>
  </div>
  <div style="background:#08090b;color:#bec2c8;border:1px solid #363a40;padding:0.8rem;font:0.9rem ui-monospace,SFMono-Regular,Menlo,Consolas,monospace">
    <div style="color:#8a929d;font-weight:700">MIDNIGHT SYSOP</div>
    <div style="color:#9aa1aa">Theme: midnight</div>
    <div style="margin-top:0.6rem"><span style="color:#8a929d">></span> Nodes <span style="color:#b8b8b8">03 Online</span></div>
    <div>  Users <span style="color:#969696">2 Pending</span></div>
    <div>  Doors <span style="color:#d2d2d2">1 Alert</span></div>
  </div>
  <div style="background:#000;color:#fff;border:1px solid #808080;padding:0.8rem;font:0.9rem ui-monospace,SFMono-Regular,Menlo,Consolas,monospace">
    <div style="color:#ffff00;font-weight:700">HIGH CONTRAST SYSOP</div>
    <div style="color:#ffffe0">Theme: high-contrast</div>
    <div style="margin-top:0.6rem"><span style="color:#ffff00">></span> Nodes <span style="color:#008000">03 Online</span></div>
    <div>  Users <span style="color:#ffff00">2 Pending</span></div>
    <div>  Doors <span style="color:#ff0000">1 Alert</span></div>
  </div>
</div>

## Palette Examples

These swatches show the main colors each theme applies to headings, selection,
success, warnings, danger states, labels, muted text, and borders.

### Oxide Classic

| Role | Color |
| --- | --- |
| Background | <span style="display:inline-block;width:1.25em;height:1.25em;background:#141414;border:1px solid #aaa;vertical-align:middle"></span> `#141414` |
| Foreground | <span style="display:inline-block;width:1.25em;height:1.25em;background:#dcdcdc;border:1px solid #555;vertical-align:middle"></span> `#dcdcdc` |
| Accent / Focus | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ff8c00;border:1px solid #555;vertical-align:middle"></span> `#ff8c00` |
| Success | <span style="display:inline-block;width:1.25em;height:1.25em;background:#00c800;border:1px solid #555;vertical-align:middle"></span> `#00c800` |
| Warning | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ffb400;border:1px solid #555;vertical-align:middle"></span> `#ffb400` |
| Danger | <span style="display:inline-block;width:1.25em;height:1.25em;background:#dc3232;border:1px solid #555;vertical-align:middle"></span> `#dc3232` |
| Label / Muted | <span style="display:inline-block;width:1.25em;height:1.25em;background:#a0a0a0;border:1px solid #555;vertical-align:middle"></span> `#a0a0a0` / <span style="display:inline-block;width:1.25em;height:1.25em;background:#646464;border:1px solid #555;vertical-align:middle"></span> `#646464` |

### Wildcat

| Role | Color |
| --- | --- |
| Background | <span style="display:inline-block;width:1.25em;height:1.25em;background:#0c0c0c;border:1px solid #aaa;vertical-align:middle"></span> `#0c0c0c` |
| Foreground | <span style="display:inline-block;width:1.25em;height:1.25em;background:#d2d2d2;border:1px solid #555;vertical-align:middle"></span> `#d2d2d2` |
| Accent / Focus | <span style="display:inline-block;width:1.25em;height:1.25em;background:#00b4f0;border:1px solid #555;vertical-align:middle"></span> `#00b4f0` |
| Success | <span style="display:inline-block;width:1.25em;height:1.25em;background:#50ff78;border:1px solid #555;vertical-align:middle"></span> `#50ff78` |
| Warning | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ffc850;border:1px solid #555;vertical-align:middle"></span> `#ffc850` |
| Danger | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ff4040;border:1px solid #555;vertical-align:middle"></span> `#ff4040` |
| Label / Muted | <span style="display:inline-block;width:1.25em;height:1.25em;background:#969696;border:1px solid #555;vertical-align:middle"></span> `#969696` / <span style="display:inline-block;width:1.25em;height:1.25em;background:#60606e;border:1px solid #555;vertical-align:middle"></span> `#60606e` |

### Telegard

| Role | Color |
| --- | --- |
| Background | <span style="display:inline-block;width:1.25em;height:1.25em;background:#081226;border:1px solid #aaa;vertical-align:middle"></span> `#081226` |
| Foreground | <span style="display:inline-block;width:1.25em;height:1.25em;background:#d6dbe8;border:1px solid #555;vertical-align:middle"></span> `#d6dbe8` |
| Accent / Focus | <span style="display:inline-block;width:1.25em;height:1.25em;background:#5c9dff;border:1px solid #555;vertical-align:middle"></span> `#5c9dff` |
| Success | <span style="display:inline-block;width:1.25em;height:1.25em;background:#52d66b;border:1px solid #555;vertical-align:middle"></span> `#52d66b` |
| Warning | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ffb343;border:1px solid #555;vertical-align:middle"></span> `#ffb343` |
| Danger | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ff5c5a;border:1px solid #555;vertical-align:middle"></span> `#ff5c5a` |
| Label / Muted | <span style="display:inline-block;width:1.25em;height:1.25em;background:#9ca8c4;border:1px solid #555;vertical-align:middle"></span> `#9ca8c4` / <span style="display:inline-block;width:1.25em;height:1.25em;background:#687595;border:1px solid #555;vertical-align:middle"></span> `#687595` |

### VBBS

| Role | Color |
| --- | --- |
| Background | <span style="display:inline-block;width:1.25em;height:1.25em;background:#10120f;border:1px solid #aaa;vertical-align:middle"></span> `#10120f` |
| Foreground | <span style="display:inline-block;width:1.25em;height:1.25em;background:#dcebdc;border:1px solid #555;vertical-align:middle"></span> `#dcebdc` |
| Accent / Focus | <span style="display:inline-block;width:1.25em;height:1.25em;background:#00c6b2;border:1px solid #555;vertical-align:middle"></span> `#00c6b2` |
| Success | <span style="display:inline-block;width:1.25em;height:1.25em;background:#58f08c;border:1px solid #555;vertical-align:middle"></span> `#58f08c` |
| Warning | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ffce54;border:1px solid #555;vertical-align:middle"></span> `#ffce54` |
| Danger | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ff6565;border:1px solid #555;vertical-align:middle"></span> `#ff6565` |
| Label / Muted | <span style="display:inline-block;width:1.25em;height:1.25em;background:#a0b4a0;border:1px solid #555;vertical-align:middle"></span> `#a0b4a0` / <span style="display:inline-block;width:1.25em;height:1.25em;background:#6e8070;border:1px solid #555;vertical-align:middle"></span> `#6e8070` |

### Mystic

| Role | Color |
| --- | --- |
| Background | <span style="display:inline-block;width:1.25em;height:1.25em;background:#12121e;border:1px solid #aaa;vertical-align:middle"></span> `#12121e` |
| Foreground | <span style="display:inline-block;width:1.25em;height:1.25em;background:#d7d7f0;border:1px solid #555;vertical-align:middle"></span> `#d7d7f0` |
| Accent / Focus | <span style="display:inline-block;width:1.25em;height:1.25em;background:#9678ff;border:1px solid #555;vertical-align:middle"></span> `#9678ff` |
| Success | <span style="display:inline-block;width:1.25em;height:1.25em;background:#8cf48c;border:1px solid #555;vertical-align:middle"></span> `#8cf48c` |
| Warning | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ffd861;border:1px solid #555;vertical-align:middle"></span> `#ffd861` |
| Danger | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ff6c74;border:1px solid #555;vertical-align:middle"></span> `#ff6c74` |
| Label / Muted | <span style="display:inline-block;width:1.25em;height:1.25em;background:#aaaacc;border:1px solid #555;vertical-align:middle"></span> `#aaaacc` / <span style="display:inline-block;width:1.25em;height:1.25em;background:#727292;border:1px solid #555;vertical-align:middle"></span> `#727292` |

### Midnight

| Role | Color |
| --- | --- |
| Background | <span style="display:inline-block;width:1.25em;height:1.25em;background:#08090b;border:1px solid #aaa;vertical-align:middle"></span> `#08090b` |
| Foreground | <span style="display:inline-block;width:1.25em;height:1.25em;background:#bec2c8;border:1px solid #555;vertical-align:middle"></span> `#bec2c8` |
| Accent / Focus | <span style="display:inline-block;width:1.25em;height:1.25em;background:#8a929d;border:1px solid #555;vertical-align:middle"></span> `#8a929d` |
| Success | <span style="display:inline-block;width:1.25em;height:1.25em;background:#b8b8b8;border:1px solid #555;vertical-align:middle"></span> `#b8b8b8` |
| Warning | <span style="display:inline-block;width:1.25em;height:1.25em;background:#969696;border:1px solid #555;vertical-align:middle"></span> `#969696` |
| Danger | <span style="display:inline-block;width:1.25em;height:1.25em;background:#d2d2d2;border:1px solid #555;vertical-align:middle"></span> `#d2d2d2` |
| Label / Muted | <span style="display:inline-block;width:1.25em;height:1.25em;background:#9aa1aa;border:1px solid #555;vertical-align:middle"></span> `#9aa1aa` / <span style="display:inline-block;width:1.25em;height:1.25em;background:#5f666f;border:1px solid #555;vertical-align:middle"></span> `#5f666f` |

### High Contrast

| Role | Color |
| --- | --- |
| Background | <span style="display:inline-block;width:1.25em;height:1.25em;background:#000000;border:1px solid #aaa;vertical-align:middle"></span> black |
| Foreground | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ffffff;border:1px solid #555;vertical-align:middle"></span> white |
| Accent / Focus | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ffff00;border:1px solid #555;vertical-align:middle"></span> yellow |
| Success | <span style="display:inline-block;width:1.25em;height:1.25em;background:#008000;border:1px solid #555;vertical-align:middle"></span> green |
| Warning | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ffff00;border:1px solid #555;vertical-align:middle"></span> yellow |
| Danger | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ff0000;border:1px solid #555;vertical-align:middle"></span> red |
| Label / Muted | <span style="display:inline-block;width:1.25em;height:1.25em;background:#ffffe0;border:1px solid #555;vertical-align:middle"></span> light yellow / <span style="display:inline-block;width:1.25em;height:1.25em;background:#808080;border:1px solid #555;vertical-align:middle"></span> gray |

## Notes

- Theme choice affects the local sysop TUI only. It does not change caller ANSI
  screens or remote caller menus.
- Theme selection is a command-line option today; it is not stored in
  `oxidebbs.toml`.
- `high-contrast` is the best starting point for terminals or displays where
  subtle muted colors are hard to distinguish.
