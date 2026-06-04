use std::collections::HashMap;
use std::io::{Read, Write};

use crate::error::BinkpError;
use crate::frame::{BinkpFrame, M_ADR, M_ERR, M_NUL, M_OK, M_PWD, read_frame, write_frame};

/// Client-side BinkP handshake data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinkpClientHandshake {
    pub addresses: Vec<String>,
    pub password: Option<String>,
}

impl BinkpClientHandshake {
    /// Create a client handshake for one or more FTN addresses.
    #[must_use]
    pub fn new(addresses: impl IntoIterator<Item = String>, password: Option<String>) -> Self {
        Self {
            addresses: addresses.into_iter().collect(),
            password,
        }
    }
}

/// Server-side BinkP handshake policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinkpServerHandshake {
    pub allowed_addresses: Vec<String>,
    pub password: Option<String>,
    pub link_passwords: HashMap<String, String>,
}

impl BinkpServerHandshake {
    /// Create a server handshake policy.
    #[must_use]
    pub fn new(
        allowed_addresses: impl IntoIterator<Item = String>,
        password: Option<String>,
    ) -> Self {
        Self {
            allowed_addresses: allowed_addresses.into_iter().collect(),
            password,
            link_passwords: HashMap::new(),
        }
    }

    /// Create a server handshake policy with per-link passwords.
    #[must_use]
    pub fn with_link_passwords(
        allowed_addresses: impl IntoIterator<Item = String>,
        link_passwords: HashMap<String, String>,
    ) -> Self {
        Self {
            allowed_addresses: allowed_addresses.into_iter().collect(),
            password: None,
            link_passwords,
        }
    }
}

/// Authenticated BinkP session metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinkpSession {
    pub peer_addresses: Vec<String>,
    pub authenticated: bool,
}

pub(crate) fn send_client_handshake<W: Write>(
    writer: &mut W,
    handshake: &BinkpClientHandshake,
) -> Result<(), BinkpError> {
    if handshake.addresses.is_empty() {
        return Err(BinkpError::Protocol(
            "BinkP client handshake requires at least one address".to_string(),
        ));
    }
    write_frame(
        writer,
        &BinkpFrame::command(M_ADR, handshake.addresses.join(" ").into_bytes()),
    )?;
    if let Some(password) = &handshake.password {
        write_frame(
            writer,
            &BinkpFrame::command(M_PWD, password.as_bytes().to_vec()),
        )?;
    }
    Ok(())
}

pub(crate) fn read_server_handshake_response<R: Read>(
    reader: &mut R,
) -> Result<BinkpSession, BinkpError> {
    let frame = read_frame(reader)?;
    if frame.frame_type != crate::frame::FrameType::Command {
        return Err(BinkpError::Protocol(
            "BinkP server sent data before handshake completed".to_string(),
        ));
    }
    match frame.command {
        M_OK => Ok(BinkpSession {
            peer_addresses: Vec::new(),
            authenticated: true,
        }),
        M_ERR => Err(BinkpError::ConnectionRefused),
        command => Err(BinkpError::Protocol(format!(
            "unexpected BinkP handshake command {command}"
        ))),
    }
}

pub(crate) fn accept_client_handshake<S: Read + Write>(
    stream: &mut S,
    policy: &BinkpServerHandshake,
) -> Result<BinkpSession, BinkpError> {
    if policy.allowed_addresses.is_empty() {
        return Err(BinkpError::Protocol(
            "BinkP server handshake requires at least one allowed address".to_string(),
        ));
    }

    let mut peer_addresses = Vec::new();
    let mut peer_password = None;

    for _ in 0..8 {
        let frame = read_frame(stream)?;
        if frame.frame_type != crate::frame::FrameType::Command {
            send_handshake_error(stream)?;
            return Err(BinkpError::Protocol(
                "BinkP peer sent data before handshake completed".to_string(),
            ));
        }

        match frame.command {
            M_ADR => {
                peer_addresses = parse_addresses(&frame.payload)?;
                if policy.password.is_none() {
                    break;
                }
            }
            M_PWD => {
                peer_password = Some(payload_text(&frame.payload)?);
                if !peer_addresses.is_empty() {
                    break;
                }
            }
            M_NUL => {}
            command => {
                send_handshake_error(stream)?;
                return Err(BinkpError::Protocol(format!(
                    "unexpected BinkP handshake command {command}"
                )));
            }
        }
    }

    if peer_addresses.is_empty()
        || !peer_addresses.iter().any(|address| {
            policy
                .allowed_addresses
                .iter()
                .any(|allowed| allowed == address)
        })
    {
        send_handshake_error(stream)?;
        return Err(BinkpError::ConnectionRefused);
    }

    let expected_password = if !policy.link_passwords.is_empty() {
        peer_addresses
            .iter()
            .find_map(|addr| policy.link_passwords.get(addr).cloned())
    } else {
        policy.password.clone()
    };

    if expected_password.as_deref() != peer_password.as_deref() {
        send_handshake_error(stream)?;
        return Err(BinkpError::ConnectionRefused);
    }

    write_frame(stream, &BinkpFrame::command(M_OK, b"ok".to_vec()))?;
    Ok(BinkpSession {
        peer_addresses,
        authenticated: true,
    })
}

fn parse_addresses(payload: &[u8]) -> Result<Vec<String>, BinkpError> {
    let text = payload_text(payload)?;
    let addresses: Vec<_> = text.split_whitespace().map(ToOwned::to_owned).collect();
    if addresses.is_empty() {
        return Err(BinkpError::Protocol(
            "BinkP M_ADR command did not contain an address".to_string(),
        ));
    }
    Ok(addresses)
}

fn payload_text(payload: &[u8]) -> Result<String, BinkpError> {
    String::from_utf8(payload.to_vec())
        .map_err(|_| BinkpError::Protocol("BinkP command payload is not UTF-8".to_string()))
}

fn send_handshake_error<W: Write>(writer: &mut W) -> Result<(), BinkpError> {
    write_frame(
        writer,
        &BinkpFrame::command(M_ERR, b"authentication failed".to_vec()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Cursor};

    struct ScriptStream {
        reads: Cursor<Vec<u8>>,
        writes: Vec<u8>,
    }

    impl ScriptStream {
        fn new(reads: Vec<u8>) -> Self {
            Self {
                reads: Cursor::new(reads),
                writes: Vec::new(),
            }
        }
    }

    impl Read for ScriptStream {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.reads.read(buf)
        }
    }

    impl Write for ScriptStream {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.writes.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn server_accepts_matching_address_and_password() {
        let mut input = Vec::new();
        write_frame(
            &mut input,
            &BinkpFrame::command(M_ADR, b"1:105/42".to_vec()),
        )
        .expect("write adr");
        write_frame(&mut input, &BinkpFrame::command(M_PWD, b"secret".to_vec()))
            .expect("write pwd");
        let mut stream = ScriptStream::new(input);
        let policy =
            BinkpServerHandshake::new(vec!["1:105/42".to_string()], Some("secret".to_string()));

        let session = accept_client_handshake(&mut stream, &policy).expect("accept handshake");

        assert!(session.authenticated);
        assert_eq!(session.peer_addresses, vec!["1:105/42".to_string()]);
        let response = read_frame(&mut stream.writes.as_slice()).expect("read response");
        assert_eq!(response.command, M_OK);
    }

    #[test]
    fn server_refuses_wrong_password_without_echoing_secret() {
        let mut input = Vec::new();
        write_frame(
            &mut input,
            &BinkpFrame::command(M_ADR, b"1:105/42".to_vec()),
        )
        .expect("write adr");
        write_frame(&mut input, &BinkpFrame::command(M_PWD, b"wrong".to_vec())).expect("write pwd");
        let mut stream = ScriptStream::new(input);
        let policy =
            BinkpServerHandshake::new(vec!["1:105/42".to_string()], Some("secret".to_string()));

        let error = accept_client_handshake(&mut stream, &policy).expect_err("refuse handshake");

        assert!(matches!(error, BinkpError::ConnectionRefused));
        assert!(!format!("{error}").contains("secret"));
        let response = read_frame(&mut stream.writes.as_slice()).expect("read response");
        assert_eq!(response.command, M_ERR);
    }

    #[test]
    fn client_writes_address_and_password() {
        let mut stream = ScriptStream::new(Vec::new());
        let handshake =
            BinkpClientHandshake::new(vec!["1:105/42".to_string()], Some("secret".to_string()));

        send_client_handshake(&mut stream, &handshake).expect("send handshake");

        let mut written = stream.writes.as_slice();
        let adr = read_frame(&mut written).expect("read adr");
        let pwd = read_frame(&mut written).expect("read pwd");
        assert_eq!(adr.command, M_ADR);
        assert_eq!(adr.payload, b"1:105/42");
        assert_eq!(pwd.command, M_PWD);
    }
}
