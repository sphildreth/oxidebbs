#![doc = "Protocol-neutral networking types shared by network transports."]

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// An FTN-style network address, represented as `zone:net/node[.point]`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FtnAddress {
    pub zone: u16,
    pub net: u16,
    pub node: u16,
    pub point: Option<u16>,
}

impl fmt::Display for FtnAddress {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.point {
            Some(point) => write!(
                formatter,
                "{}:{}/{}.{}",
                self.zone, self.net, self.node, point
            ),
            None => write!(formatter, "{}:{}/{}", self.zone, self.net, self.node),
        }
    }
}

impl FromStr for FtnAddress {
    type Err = NetworkAddressError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let (zone, rest) = raw
            .split_once(':')
            .ok_or(NetworkAddressError::MissingZone)?;
        let (net, node_and_point) = rest
            .split_once('/')
            .ok_or(NetworkAddressError::MissingNet)?;
        let (node, point) = match node_and_point.split_once('.') {
            Some((node, point)) => (node, Some(parse_part(point)?)),
            None => (node_and_point, None),
        };

        Ok(Self {
            zone: parse_part(zone)?,
            net: parse_part(net)?,
            node: parse_part(node)?,
            point,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum NetworkAddressError {
    #[error("FTN address is missing zone")]
    MissingZone,

    #[error("FTN address is missing net")]
    MissingNet,

    #[error("invalid numeric FTN address part")]
    InvalidPart,
}

fn parse_part(raw: &str) -> Result<u16, NetworkAddressError> {
    raw.parse::<u16>()
        .map_err(|_| NetworkAddressError::InvalidPart)
}

/// Supported network adapter families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NetworkAdapter {
    /// Legacy FidoNet-compatible FTN networks.
    LegacyFtn,
    /// First-party OxideNet profile.
    OxideNet,
}

impl NetworkAdapter {
    /// Stable config/database label for this adapter.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LegacyFtn => "legacy-ftn",
            Self::OxideNet => "oxidenet",
        }
    }
}

impl fmt::Display for NetworkAdapter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for NetworkAdapter {
    type Err = NetworkConfigError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "legacy-ftn" => Ok(Self::LegacyFtn),
            "oxidenet" => Ok(Self::OxideNet),
            _ => Err(NetworkConfigError::UnknownAdapter(raw.to_string())),
        }
    }
}

/// Per-link bundle compression policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NetworkCompression {
    /// Send raw packets without archive compression.
    None,
    /// Use ZIP bundles.
    Zip,
    /// Use ARJ bundles.
    Arj,
}

impl NetworkCompression {
    /// Stable config/database label for this compression policy.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Zip => "zip",
            Self::Arj => "arj",
        }
    }
}

impl fmt::Display for NetworkCompression {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for NetworkCompression {
    type Err = NetworkConfigError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "none" => Ok(Self::None),
            "zip" => Ok(Self::Zip),
            "arj" => Ok(Self::Arj),
            _ => Err(NetworkConfigError::UnknownCompression(raw.to_string())),
        }
    }
}

/// Per-link transport-security policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransportSecurity {
    /// TLS is required and plaintext is rejected.
    TlsRequired,
    /// Try TLS first, then fall back to explicitly permitted plaintext.
    TlsOpportunistic,
    /// Plaintext legacy transport is allowed for legacy FTN profiles only.
    PlaintextLegacy,
}

impl TransportSecurity {
    /// Stable config/database label for this transport policy.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TlsRequired => "tls_required",
            Self::TlsOpportunistic => "tls_opportunistic",
            Self::PlaintextLegacy => "plaintext_legacy",
        }
    }
}

impl fmt::Display for TransportSecurity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for TransportSecurity {
    type Err = NetworkConfigError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw {
            "tls_required" => Ok(Self::TlsRequired),
            "tls_opportunistic" => Ok(Self::TlsOpportunistic),
            "plaintext_legacy" => Ok(Self::PlaintextLegacy),
            _ => Err(NetworkConfigError::UnknownTransportSecurity(
                raw.to_string(),
            )),
        }
    }
}

/// Errors from parsing stable network config/database labels.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum NetworkConfigError {
    /// Unknown network adapter label.
    #[error("unknown network adapter {0}")]
    UnknownAdapter(String),

    /// Unknown network compression label.
    #[error("unknown network compression {0}")]
    UnknownCompression(String),

    /// Unknown transport-security label.
    #[error("unknown transport security {0}")]
    UnknownTransportSecurity(String),
}

/// Protocol-neutral network profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkProfile {
    pub id: String,
    pub key: String,
    pub name: String,
    pub adapter: NetworkAdapter,
    pub local_address: FtnAddress,
    pub enabled: bool,
}

/// Protocol-neutral network link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkLink {
    pub id: String,
    pub key: String,
    pub network_id: String,
    pub address: FtnAddress,
    pub host: String,
    pub binkp_port: u16,
    pub poll_schedule_minutes: u16,
    pub compression: NetworkCompression,
    pub transport_security: TransportSecurity,
    pub enabled: bool,
}

/// Mapping between a local area and a network area tag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EchoMailAreaMapping {
    pub local_area_id: String,
    pub network_id: String,
    pub echo_tag: String,
    pub read_only: bool,
}

/// Message envelope as exchanged on network transports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetMailMessage {
    pub id: String,
    pub from: FtnAddress,
    pub to: FtnAddress,
    pub subject: String,
    pub body: String,
    pub created_at: String,
}

/// Network message kind used by shared imports and exports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NetworkMessageKind {
    /// Echomail area message.
    Echomail,
    /// Direct netmail message.
    Netmail,
    /// Local-only message wrapped for conversion bookkeeping.
    Local,
}

/// Local message data needed to build a network envelope without depending on
/// `oxidebbs-core`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalMessageEnvelope {
    pub id: String,
    pub area_id: String,
    pub author_display_name: String,
    pub subject: String,
    pub body: String,
    pub created_at: String,
    pub reply_to_id: Option<String>,
}

/// Protocol-neutral message envelope used at network boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkMessageEnvelope {
    pub local_message_id: Option<String>,
    pub network_id: String,
    pub kind: NetworkMessageKind,
    pub area_tag: Option<String>,
    pub origin: FtnAddress,
    pub destination: Option<FtnAddress>,
    pub from_name: String,
    pub to_name: Option<String>,
    pub subject: String,
    pub raw_text: Vec<u8>,
    pub display_body: String,
    pub msgid: Option<String>,
    pub replyid: Option<String>,
    pub created_at: String,
}

/// Converts local messages into protocol-neutral network envelopes.
pub trait IntoNetworkEnvelope {
    /// Build a network envelope for the given local network profile and area.
    fn into_network_envelope(
        self,
        network_id: impl Into<String>,
        origin: FtnAddress,
        area_tag: impl Into<String>,
    ) -> NetworkMessageEnvelope;
}

impl IntoNetworkEnvelope for LocalMessageEnvelope {
    fn into_network_envelope(
        self,
        network_id: impl Into<String>,
        origin: FtnAddress,
        area_tag: impl Into<String>,
    ) -> NetworkMessageEnvelope {
        NetworkMessageEnvelope {
            local_message_id: Some(self.id),
            network_id: network_id.into(),
            kind: NetworkMessageKind::Echomail,
            area_tag: Some(area_tag.into()),
            origin,
            destination: None,
            from_name: self.author_display_name,
            to_name: None,
            subject: self.subject,
            raw_text: self.body.as_bytes().to_vec(),
            display_body: self.body,
            msgid: None,
            replyid: self.reply_to_id,
            created_at: self.created_at,
        }
    }
}

/// Duplicate-tracking key for protocol-neutral message dedupe.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DuplicateDetectionKey {
    pub network_id: String,
    pub area_tag: String,
    pub origin: FtnAddress,
    pub message_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacketDirection {
    Import,
    Export,
}

/// Queue lifecycle state for protocol-neutral packet/message work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QueueState {
    /// Queued but not started.
    Pending,
    /// Currently being processed.
    Processing,
    /// Completed successfully.
    Processed,
    /// Quarantined for operator review.
    Quarantined,
    /// Failed processing.
    Failed,
    /// Skipped intentionally.
    Skipped,
}

/// A network boundary or endpoint pair used by adapters and polling.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketBoundary {
    pub network_id: String,
    pub direction: PacketDirection,
    pub peer: FtnAddress,
    pub spool_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ftn_address_without_point() {
        let address: FtnAddress = "1:105/42".parse().expect("parse address");

        assert_eq!(
            address,
            FtnAddress {
                zone: 1,
                net: 105,
                node: 42,
                point: None
            }
        );
        assert_eq!(address.to_string(), "1:105/42");
    }

    #[test]
    fn parses_ftn_address_with_point() {
        let address: FtnAddress = "1:105/42.7".parse().expect("parse address");

        assert_eq!(address.point, Some(7));
        assert_eq!(address.to_string(), "1:105/42.7");
    }

    #[test]
    fn models_echomail_area_mapping() {
        let mapping = EchoMailAreaMapping {
            local_area_id: "general".to_string(),
            network_id: "oxidennet".to_string(),
            echo_tag: "OXIDE.GENERAL".to_string(),
            read_only: false,
        };

        assert_eq!(mapping.echo_tag, "OXIDE.GENERAL");
    }

    #[test]
    fn duplicate_detection_key_is_stable() {
        let key = DuplicateDetectionKey {
            network_id: "oxidennet".to_string(),
            area_tag: "OXIDE.GENERAL".to_string(),
            origin: "1:105/42".parse().expect("parse"),
            message_id: "abc123".to_string(),
        };

        assert_eq!(key.origin.to_string(), "1:105/42");
    }

    #[test]
    fn packet_boundary_tracks_import_export_edge() {
        let boundary = PacketBoundary {
            network_id: "oxidennet".to_string(),
            direction: PacketDirection::Import,
            peer: "1:105/42".parse().expect("parse"),
            spool_path: "./spool/inbound".to_string(),
        };

        assert_eq!(boundary.direction, PacketDirection::Import);
    }

    #[test]
    fn parses_stable_network_labels() {
        assert_eq!(
            "legacy-ftn".parse::<NetworkAdapter>().expect("adapter"),
            NetworkAdapter::LegacyFtn
        );
        assert_eq!(
            "zip".parse::<NetworkCompression>().expect("compression"),
            NetworkCompression::Zip
        );
        assert_eq!(
            "tls_required"
                .parse::<TransportSecurity>()
                .expect("transport security"),
            TransportSecurity::TlsRequired
        );
    }

    #[test]
    fn rejects_unknown_network_labels() {
        assert!("bad".parse::<NetworkAdapter>().is_err());
        assert!("bad".parse::<NetworkCompression>().is_err());
        assert!("bad".parse::<TransportSecurity>().is_err());
    }

    #[test]
    fn converts_local_message_to_network_envelope() {
        let local = LocalMessageEnvelope {
            id: "msg-1".to_string(),
            area_id: "area-1".to_string(),
            author_display_name: "Sysop".to_string(),
            subject: "Hello".to_string(),
            body: "Body".to_string(),
            created_at: "2026-06-04T00:00:00Z".to_string(),
            reply_to_id: Some("root".to_string()),
        };

        let envelope = local.into_network_envelope(
            "network-1",
            "42:1/100".parse().expect("origin"),
            "OXIDE.GENERAL",
        );

        assert_eq!(envelope.kind, NetworkMessageKind::Echomail);
        assert_eq!(envelope.area_tag.as_deref(), Some("OXIDE.GENERAL"));
        assert_eq!(envelope.raw_text, b"Body");
        assert_eq!(envelope.replyid.as_deref(), Some("root"));
    }
}
