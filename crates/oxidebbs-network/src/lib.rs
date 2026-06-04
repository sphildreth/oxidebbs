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
}
