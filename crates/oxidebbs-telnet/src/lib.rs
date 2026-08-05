pub mod serial;
pub mod telnet;
pub mod transport;

pub use serial::{
    SerialFlowControl, SerialHandle, SerialLoopback, SerialOpenError, SerialParity,
    SerialPortConfig, SerialTransport,
};

pub use telnet::{
    DO, DONT, IAC, SB, SE, TELOPT_ECHO, TELOPT_NAWS, TELOPT_SUPPRESS_GO_AHEAD,
    TELOPT_TERMINAL_TYPE, TELOPT_TTYPE_IS, TELOPT_TTYPE_SEND, TelnetCommand, TelnetEvent,
    TelnetParser, TelnetSession, WILL, WONT,
};

pub use transport::{LoopbackTransport, TcpTransport, Transport, TransportError};
