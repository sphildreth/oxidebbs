# Serial And Modem Transport

Telnet remains enabled by default. Serial/modem transport is available
and is disabled until `[serial].enabled = true`.

When enabled, each `[[serial.devices]]` entry opens a physical TTY with the
configured baud rate, parity, stop bits, flow control, init strings, and optional
answer string. The caller session then uses the same login, menu, door, message,
and file-area flow as telnet callers, but without telnet negotiation or IAC
escaping.

`require_carrier_detect = true` makes startup fail if the adapter or platform
cannot report carrier state. `drop_dtr_on_hangup = true` drops DTR during
hangup when the platform supports it.

Keep serial disabled on systems that do not have stable device paths or modem
line-state support. Hardware smoke testing should verify login, menu input,
file transfer, and logoff through the exact TTY device the sysop plans to use.
