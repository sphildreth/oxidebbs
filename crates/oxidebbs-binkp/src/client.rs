use std::io::{Read, Write};

use crate::error::BinkpError;
use crate::handshake::{BinkpClientHandshake, BinkpSession};
use crate::transfer::{BinkpInboundFile, BinkpOutboundFile};

/// BinkP client configuration and state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinkpClient;

impl BinkpClient {
    /// Create a BinkP client.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Perform the initial BinkP client handshake on an established stream.
    ///
    /// # Errors
    ///
    /// Returns protocol errors for invalid local handshake data or unexpected
    /// peer responses, connection refusal when the server rejects the handshake,
    /// or I/O errors from the stream.
    pub fn handshake<S: Read + Write>(
        &self,
        stream: &mut S,
        handshake: &BinkpClientHandshake,
    ) -> Result<BinkpSession, BinkpError> {
        crate::handshake::send_client_handshake(stream, handshake)?;
        crate::handshake::read_server_handshake_response(stream)
    }

    /// Send one BinkP file offer and payload on an authenticated stream.
    ///
    /// # Errors
    ///
    /// Returns protocol errors for invalid file metadata or I/O errors from the
    /// stream.
    pub fn send_file<W: Write>(
        &self,
        writer: &mut W,
        file: &BinkpOutboundFile,
    ) -> Result<(), BinkpError> {
        crate::transfer::send_file(writer, file)
    }

    /// Receive the next BinkP file, or `None` when the peer ends the batch.
    ///
    /// # Errors
    ///
    /// Returns protocol errors for malformed file exchange or I/O errors from
    /// the stream.
    pub fn receive_next_file<S: Read + Write>(
        &self,
        stream: &mut S,
    ) -> Result<Option<BinkpInboundFile>, BinkpError> {
        crate::transfer::receive_next_file(stream)
    }

    /// Send the BinkP end-of-batch marker.
    ///
    /// # Errors
    ///
    /// Returns I/O errors from the stream.
    pub fn send_end_of_batch<W: Write>(&self, writer: &mut W) -> Result<(), BinkpError> {
        crate::transfer::send_end_of_batch(writer)
    }
}

impl Default for BinkpClient {
    fn default() -> Self {
        Self::new()
    }
}
