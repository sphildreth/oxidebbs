# Serial And Modem Transport

Telnet remains enabled by default. Serial/modem transport is available
and is disabled until `[serial].enabled = true`.

When enabled, each `[[serial.devices]]` entry opens a physical TTY with the
configured baud rate, parity, stop bits, flow control, init strings, and optional
answer string. The caller session then uses the same login, menu, door, message,
and file-area flow as telnet callers, but without telnet negotiation or IAC
escaping.

Example:

```toml
[serial]
enabled = true

[[serial.devices]]
name = "modem"
path = "/dev/ttyUSB0"
baud_rate = 115200
data_bits = 8
parity = "none"
stop_bits = 1
flow_control = "rtscts"
init_strings = ["ATZ"]
answer_string = "ATA"
require_carrier_detect = false
drop_dtr_on_hangup = true
read_timeout_ms = 100
```

Validation requires at least one device when serial is enabled. Device names and
paths must be nonblank and unique, `data_bits` must be `5`, `6`, `7`, or `8`,
`parity` must be `none`, `odd`, or `even`, `stop_bits` must be `1` or `2`,
`flow_control` must be `none`, `rtscts`, or `xonxoff`, and
`read_timeout_ms` must be greater than zero.

`require_carrier_detect = true` makes startup fail if the adapter or platform
cannot report carrier state. `drop_dtr_on_hangup = true` drops DTR during
hangup when the platform supports it.

Keep serial disabled on systems that do not have stable device paths or modem
line-state support. Hardware smoke testing should verify login, menu input,
file transfer, and logoff through the exact TTY device the sysop plans to use.
