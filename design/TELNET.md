# Telnet Design

## v1 goal

Provide stable telnet access for classic BBS clients, especially SyncTERM-style clients.

## Required capabilities

- Accept TCP connections
- Assign a node
- Handle IAC command bytes
- Negotiate basic telnet options
- Normalize CR/LF input
- Detect disconnect
- Enforce idle timeout
- Clean up node/session state

## Initial telnet options

Start small:

- Suppress Go Ahead
- Echo behavior
- Terminal type later
- NAWS/window size later

## Transport abstraction

The BBS session should not know whether the caller is telnet or serial.

```rust
pub trait Transport {
    async fn read_byte(&mut self) -> Result<Option<u8>>;
    async fn write_all(&mut self, bytes: &[u8]) -> Result<()>;
    async fn hangup(&mut self) -> Result<()>;
}
```

## Testing

Use a loopback transport for deterministic tests.
