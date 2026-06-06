use std::io::{Read, Write};

use crate::error::BinkpError;
use crate::handshake::{BinkpServerHandshake, BinkpSession};
use crate::transfer::{BinkpInboundFile, BinkpOutboundFile};

/// BinkP server configuration and state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BinkpServer;

impl BinkpServer {
    /// Create a BinkP server.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Accept the initial BinkP server handshake on an established stream.
    ///
    /// # Errors
    ///
    /// Returns protocol errors for invalid local policy or malformed peer
    /// commands, connection refusal for address/password mismatches, or I/O
    /// errors from the stream.
    pub fn accept_handshake<S: Read + Write>(
        &self,
        stream: &mut S,
        policy: &BinkpServerHandshake,
    ) -> Result<BinkpSession, BinkpError> {
        crate::handshake::accept_client_handshake(stream, policy)
    }

    /// Accept the initial BinkP server handshake and enforce whether the
    /// established transport satisfies any per-link TLS requirements.
    ///
    /// # Errors
    ///
    /// Returns protocol errors for invalid local policy or malformed peer
    /// commands, connection refusal for address/password mismatches,
    /// `TlsRequired` when a TLS-required link arrives over plaintext, or I/O
    /// errors from the stream.
    pub fn accept_handshake_with_transport_security<S: Read + Write>(
        &self,
        stream: &mut S,
        policy: &BinkpServerHandshake,
        secure_transport: bool,
    ) -> Result<BinkpSession, BinkpError> {
        crate::handshake::accept_client_handshake_with_transport_security(
            stream,
            policy,
            secure_transport,
        )
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

    /// Send a complete BinkP batch and terminate it with `M_EOB`.
    ///
    /// # Errors
    ///
    /// Returns protocol errors for invalid file metadata or I/O errors from the
    /// stream.
    pub fn send_batch<W: Write>(
        &self,
        writer: &mut W,
        files: &[BinkpOutboundFile],
    ) -> Result<(), BinkpError> {
        crate::transfer::send_batch(writer, files)
    }

    /// Send a complete BinkP batch, waiting for one `M_GOT` acknowledgement per
    /// file before terminating the batch with `M_EOB`.
    ///
    /// # Errors
    ///
    /// Returns protocol errors for invalid metadata, bad acknowledgements, or
    /// I/O errors from the stream.
    pub fn send_batch_with_acknowledgements<S: Read + Write>(
        &self,
        stream: &mut S,
        files: &[BinkpOutboundFile],
    ) -> Result<(), BinkpError> {
        crate::transfer::send_batch_with_acknowledgements(stream, files)
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

    /// Receive BinkP files until the peer sends `M_EOB`.
    ///
    /// # Errors
    ///
    /// Returns protocol errors for malformed file exchange or I/O errors from
    /// the stream.
    pub fn receive_batch<S: Read + Write>(
        &self,
        stream: &mut S,
    ) -> Result<Vec<BinkpInboundFile>, BinkpError> {
        crate::transfer::receive_batch(stream)
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

impl Default for BinkpServer {
    fn default() -> Self {
        Self::new()
    }
}
