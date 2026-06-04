use thiserror::Error;

use oxidebbs_network::{FtnAddress, NetworkAddressError};

pub const OXIDENET_ZONE: i32 = 42;
pub const DEFAULT_HUB_ADDRESS: (&str, i32, i32, i32) = ("42", 1, 1, 0);
pub const DEFAULT_BACKUP_HUB: (&str, i32, i32, i32) = ("42", 1, 2, 0);
pub const INFRA_RANGE_START: i32 = 10;
pub const INFRA_RANGE_END: i32 = 99;
pub const MEMBER_RANGE_START: i32 = 100;
pub const TEST_LAB_START: i32 = 900;
pub const OXIDENET_ZONE_U16: u16 = 42;
pub const OXIDENET_PRIMARY_NET: u16 = 1;
pub const DEFAULT_HUB_NODE: u16 = 1;
pub const DEFAULT_BACKUP_HUB_NODE: u16 = 2;
pub const INFRA_RANGE_START_U16: u16 = 10;
pub const INFRA_RANGE_END_U16: u16 = 99;
pub const MEMBER_RANGE_START_U16: u16 = 100;
pub const MEMBER_RANGE_END_U16: u16 = 899;
pub const TEST_LAB_START_U16: u16 = 900;
pub const DEFAULT_POLL_INTERVAL_MINUTES: u16 = 60;

#[derive(Debug, Error)]
pub enum OxideNetError {
    #[error("network error: {0}")]
    Network(String),

    #[error("application already exists for address {0}")]
    DuplicateApplication(String),

    #[error("node not found: {0}")]
    NodeNotFound(String),

    #[error("invalid config package: {0}")]
    InvalidConfigPackage(String),

    #[error("invalid OxideNet address {address:?}: {reason}")]
    InvalidAddress { address: String, reason: String },
}

pub const DEFAULT_AREAS: &[&str] = &[
    "OXIDE.GENERAL",
    "OXIDE.SYSOP",
    "OXIDE.NETWORK",
    "OXIDE.TEST",
];

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Application {
    pub id: String,
    pub sysop_name: String,
    pub board_name: String,
    pub email: String,
    pub description: String,
    pub requested_address: Option<String>,
    pub assigned_address: Option<String>,
    pub status: ApplicationStatus,
    pub created_at: String,
    pub reviewed_at: Option<String>,
    pub reviewed_by: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApplicationStatus {
    Draft,
    #[serde(alias = "pending")]
    Submitted,
    #[serde(alias = "needs_info")]
    NeedsInfo,
    Approved,
    #[serde(alias = "config_generated")]
    ConfigGenerated,
    #[serde(alias = "first_poll_pending")]
    FirstPollPending,
    Active,
    Probation,
    Suspended,
    Retired,
    Rejected,
    Withdrawn,
    #[serde(alias = "on_hold", alias = "on-hold", alias = "hold")]
    NeedsReviewHold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OxideNetAddressClass {
    PrimaryHub,
    BackupHub,
    Infrastructure,
    Member,
    TestLab,
    FutureNet,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OxideNode {
    pub id: String,
    pub address: String,
    pub sysop_name: String,
    pub board_name: String,
    pub host: String,
    pub binkp_port: u16,
    pub password_hash: String,
    pub suspended: bool,
    pub created_at: String,
    pub last_poll_at: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ConfigPackage {
    pub oxidenet: OxideNetToml,
    pub areas: AreasToml,
    pub nodelist: NodelistToml,
    pub credentials: CredentialsToml,
    pub generated_at: String,
    pub token_hash: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OxideNetToml {
    pub network: PackageNetwork,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PackageNetwork {
    pub key: String,
    pub name: String,
    pub address: String,
    pub hub_address: String,
    pub local: PackageLocal,
    pub hub: PackageHub,
    pub auth: PackageAuth,
    pub policy: PackagePolicy,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PackageLocal {
    pub board_name: String,
    pub sysop_alias: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PackageHub {
    pub name: String,
    pub host: String,
    pub binkp_port: u16,
    pub poll_interval_minutes: u16,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PackageAuth {
    pub session_password: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PackagePolicy {
    pub accepted_policy_version: String,
    pub accepted_at: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct AreasToml {
    pub areas: Vec<PackageArea>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PackageArea {
    pub tag: String,
    pub local_key: String,
    pub name: String,
    pub subscribed: bool,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct NodelistToml {
    pub nodes: Vec<PackageNode>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PackageNode {
    pub address: String,
    pub board_name: String,
    pub sysop_alias: String,
    pub host: String,
    pub binkp_port: u16,
    pub status: String,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CredentialsToml {
    pub address: String,
    pub hub_address: String,
    pub session_password: String,
}

#[must_use]
pub const fn primary_hub_address() -> FtnAddress {
    FtnAddress {
        zone: OXIDENET_ZONE_U16,
        net: OXIDENET_PRIMARY_NET,
        node: DEFAULT_HUB_NODE,
        point: None,
    }
}

#[must_use]
pub const fn backup_hub_address() -> FtnAddress {
    FtnAddress {
        zone: OXIDENET_ZONE_U16,
        net: OXIDENET_PRIMARY_NET,
        node: DEFAULT_BACKUP_HUB_NODE,
        point: None,
    }
}

pub fn parse_oxidenet_address(raw: &str) -> Result<FtnAddress, OxideNetError> {
    let address = raw
        .parse::<FtnAddress>()
        .map_err(|error: NetworkAddressError| OxideNetError::InvalidAddress {
            address: raw.to_string(),
            reason: error.to_string(),
        })?;
    classify_oxidenet_address(&address)?;
    Ok(address)
}

pub fn classify_oxidenet_address(
    address: &FtnAddress,
) -> Result<OxideNetAddressClass, OxideNetError> {
    if address.zone != OXIDENET_ZONE_U16 {
        return Err(address_error(address, "zone must be 42"));
    }
    if address.node == 0 {
        return Err(address_error(address, "node number must be non-zero"));
    }

    if address.net != OXIDENET_PRIMARY_NET {
        return Ok(OxideNetAddressClass::FutureNet);
    }

    match address.node {
        DEFAULT_HUB_NODE => Ok(OxideNetAddressClass::PrimaryHub),
        DEFAULT_BACKUP_HUB_NODE => Ok(OxideNetAddressClass::BackupHub),
        INFRA_RANGE_START_U16..=INFRA_RANGE_END_U16 => Ok(OxideNetAddressClass::Infrastructure),
        MEMBER_RANGE_START_U16..=MEMBER_RANGE_END_U16 => Ok(OxideNetAddressClass::Member),
        TEST_LAB_START_U16..=u16::MAX => Ok(OxideNetAddressClass::TestLab),
        _ => Err(address_error(
            address,
            "node is reserved for future hub/infrastructure assignment",
        )),
    }
}

#[must_use]
pub fn is_assignable_member_address(address: &FtnAddress) -> bool {
    matches!(
        classify_oxidenet_address(address),
        Ok(OxideNetAddressClass::Member)
    ) && address.point.is_none()
}

#[must_use]
pub fn is_test_lab_address(address: &FtnAddress) -> bool {
    matches!(
        classify_oxidenet_address(address),
        Ok(OxideNetAddressClass::TestLab)
    ) && address.point.is_none()
}

pub fn next_member_address<'a>(
    used_addresses: impl IntoIterator<Item = &'a FtnAddress>,
) -> Result<FtnAddress, OxideNetError> {
    let used = used_addresses
        .into_iter()
        .filter(|address| {
            address.zone == OXIDENET_ZONE_U16
                && address.net == OXIDENET_PRIMARY_NET
                && address.point.is_none()
        })
        .map(|address| address.node)
        .collect::<std::collections::BTreeSet<_>>();

    for node in MEMBER_RANGE_START_U16..=MEMBER_RANGE_END_U16 {
        if !used.contains(&node) {
            return Ok(FtnAddress {
                zone: OXIDENET_ZONE_U16,
                net: OXIDENET_PRIMARY_NET,
                node,
                point: None,
            });
        }
    }

    Err(OxideNetError::InvalidAddress {
        address: format!("{OXIDENET_ZONE_U16}:{OXIDENET_PRIMARY_NET}/*"),
        reason: "no member addresses remain in 42:1/100-899".to_string(),
    })
}

impl ConfigPackage {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        if self.generated_at.trim().is_empty() {
            return Err(package_error("generated_at is required"));
        }
        if self.token_hash.trim().is_empty() {
            return Err(package_error("token_hash is required"));
        }
        self.oxidenet.validate()?;
        self.areas.validate()?;
        self.nodelist.validate()?;
        self.credentials.validate_against(&self.oxidenet.network)?;
        Ok(())
    }
}

impl OxideNetToml {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        self.network.validate()
    }
}

impl PackageNetwork {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        if self.key != "oxidenet" {
            return Err(package_error("network key must be oxidenet"));
        }
        if self.name.trim().is_empty() {
            return Err(package_error("network name is required"));
        }
        let local = parse_oxidenet_address(&self.address)?;
        if !is_assignable_member_address(&local) && !is_test_lab_address(&local) {
            return Err(package_error(
                "network address must be an assignable member or test-lab address",
            ));
        }
        let hub = parse_oxidenet_address(&self.hub_address)?;
        if !matches!(
            classify_oxidenet_address(&hub)?,
            OxideNetAddressClass::PrimaryHub
                | OxideNetAddressClass::BackupHub
                | OxideNetAddressClass::Infrastructure
        ) {
            return Err(package_error("hub address must identify an OxideNet hub"));
        }
        self.local.validate()?;
        self.hub.validate()?;
        self.auth.validate()?;
        self.policy.validate()?;
        Ok(())
    }
}

impl PackageLocal {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        if self.board_name.trim().is_empty() {
            return Err(package_error("local board_name is required"));
        }
        if self.sysop_alias.trim().is_empty() {
            return Err(package_error("local sysop_alias is required"));
        }
        Ok(())
    }
}

impl PackageHub {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        if self.name.trim().is_empty() {
            return Err(package_error("hub name is required"));
        }
        if self.host.trim().is_empty() {
            return Err(package_error("hub host is required"));
        }
        if self.binkp_port == 0 {
            return Err(package_error("hub binkp_port must be non-zero"));
        }
        if self.poll_interval_minutes == 0 {
            return Err(package_error("hub poll_interval_minutes must be non-zero"));
        }
        Ok(())
    }
}

impl PackageAuth {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        if self.session_password.trim().is_empty() {
            return Err(package_error("session_password is required"));
        }
        Ok(())
    }
}

impl PackagePolicy {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        if self.accepted_policy_version.trim().is_empty() {
            return Err(package_error("accepted_policy_version is required"));
        }
        if self.accepted_at.trim().is_empty() {
            return Err(package_error("accepted_at is required"));
        }
        Ok(())
    }
}

impl AreasToml {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        if self.areas.is_empty() {
            return Err(package_error("at least one area is required"));
        }
        for area in &self.areas {
            area.validate()?;
        }
        Ok(())
    }
}

impl PackageArea {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        validate_area_tag(&self.tag)?;
        if self.local_key.trim().is_empty() {
            return Err(package_error("area local_key is required"));
        }
        if self.name.trim().is_empty() {
            return Err(package_error("area name is required"));
        }
        Ok(())
    }
}

impl NodelistToml {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        if self.nodes.is_empty() {
            return Err(package_error("at least one nodelist node is required"));
        }
        for node in &self.nodes {
            node.validate()?;
        }
        Ok(())
    }
}

impl PackageNode {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        parse_oxidenet_address(&self.address)?;
        if self.board_name.trim().is_empty() {
            return Err(package_error("nodelist board_name is required"));
        }
        if self.sysop_alias.trim().is_empty() {
            return Err(package_error("nodelist sysop_alias is required"));
        }
        if self.host.trim().is_empty() {
            return Err(package_error("nodelist host is required"));
        }
        if self.binkp_port == 0 {
            return Err(package_error("nodelist binkp_port must be non-zero"));
        }
        if self.status.trim().is_empty() {
            return Err(package_error("nodelist status is required"));
        }
        Ok(())
    }
}

impl CredentialsToml {
    pub fn validate_against(&self, network: &PackageNetwork) -> Result<(), OxideNetError> {
        if self.address != network.address {
            return Err(package_error(
                "credentials address must match oxidenet network address",
            ));
        }
        if self.hub_address != network.hub_address {
            return Err(package_error(
                "credentials hub_address must match oxidenet hub_address",
            ));
        }
        if self.session_password != network.auth.session_password {
            return Err(package_error(
                "credentials session_password must match network auth password",
            ));
        }
        PackageAuth {
            session_password: self.session_password.clone(),
        }
        .validate()
    }
}

pub fn validate_area_tag(tag: &str) -> Result<(), OxideNetError> {
    if tag.is_empty() {
        return Err(package_error("area tag is required"));
    }
    if tag != tag.to_ascii_uppercase() {
        return Err(package_error("area tag must be uppercase ASCII"));
    }
    if !tag.bytes().all(|byte| {
        byte.is_ascii_uppercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
    }) {
        return Err(package_error(
            "area tag may contain only uppercase ASCII letters, digits, '.', '_', or '-'",
        ));
    }
    Ok(())
}

fn address_error(address: &FtnAddress, reason: &str) -> OxideNetError {
    OxideNetError::InvalidAddress {
        address: address.to_string(),
        reason: reason.to_string(),
    }
}

fn package_error(message: &str) -> OxideNetError {
    OxideNetError::InvalidConfigPackage(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(raw: &str) -> FtnAddress {
        raw.parse().expect("parse address")
    }

    fn valid_package() -> ConfigPackage {
        ConfigPackage {
            oxidenet: OxideNetToml {
                network: PackageNetwork {
                    key: "oxidenet".to_string(),
                    name: "OxideNet".to_string(),
                    address: "42:1/100".to_string(),
                    hub_address: "42:1/1".to_string(),
                    local: PackageLocal {
                        board_name: "Retro Cavern BBS".to_string(),
                        sysop_alias: "Night Owl".to_string(),
                    },
                    hub: PackageHub {
                        name: "Blackboard BBS".to_string(),
                        host: "blackboard.example.net".to_string(),
                        binkp_port: 24554,
                        poll_interval_minutes: DEFAULT_POLL_INTERVAL_MINUTES,
                    },
                    auth: PackageAuth {
                        session_password: "generated-secret".to_string(),
                    },
                    policy: PackagePolicy {
                        accepted_policy_version: "1.0".to_string(),
                        accepted_at: "2026-06-01T00:00:00Z".to_string(),
                    },
                },
            },
            areas: AreasToml {
                areas: DEFAULT_AREAS
                    .iter()
                    .map(|tag| PackageArea {
                        tag: (*tag).to_string(),
                        local_key: tag.to_ascii_lowercase().replace('.', "-"),
                        name: format!("OxideNet {tag}"),
                        subscribed: true,
                    })
                    .collect(),
            },
            nodelist: NodelistToml {
                nodes: vec![
                    PackageNode {
                        address: "42:1/1".to_string(),
                        board_name: "Blackboard BBS".to_string(),
                        sysop_alias: "Sysop".to_string(),
                        host: "blackboard.example.net".to_string(),
                        binkp_port: 24554,
                        status: "active".to_string(),
                    },
                    PackageNode {
                        address: "42:1/100".to_string(),
                        board_name: "Retro Cavern BBS".to_string(),
                        sysop_alias: "Night Owl".to_string(),
                        host: "retro.example.net".to_string(),
                        binkp_port: 24554,
                        status: "first-poll-pending".to_string(),
                    },
                ],
            },
            credentials: CredentialsToml {
                address: "42:1/100".to_string(),
                hub_address: "42:1/1".to_string(),
                session_password: "generated-secret".to_string(),
            },
            generated_at: "2026-06-01T00:00:00Z".to_string(),
            token_hash: "$argon2id$hash".to_string(),
        }
    }

    #[test]
    fn classifies_primary_oxidenet_ranges() {
        assert_eq!(
            classify_oxidenet_address(&addr("42:1/1")).expect("classify"),
            OxideNetAddressClass::PrimaryHub
        );
        assert_eq!(
            classify_oxidenet_address(&addr("42:1/2")).expect("classify"),
            OxideNetAddressClass::BackupHub
        );
        assert_eq!(
            classify_oxidenet_address(&addr("42:1/10")).expect("classify"),
            OxideNetAddressClass::Infrastructure
        );
        assert_eq!(
            classify_oxidenet_address(&addr("42:1/100")).expect("classify"),
            OxideNetAddressClass::Member
        );
        assert_eq!(
            classify_oxidenet_address(&addr("42:1/900")).expect("classify"),
            OxideNetAddressClass::TestLab
        );
        assert_eq!(
            classify_oxidenet_address(&addr("42:2/1")).expect("classify"),
            OxideNetAddressClass::FutureNet
        );
    }

    #[test]
    fn rejects_invalid_or_reserved_addresses() {
        assert!(parse_oxidenet_address("41:1/100").is_err());
        assert!(parse_oxidenet_address("42:1/0").is_err());
        assert!(parse_oxidenet_address("42:1/3").is_err());
        assert!(parse_oxidenet_address("invalid node").is_err());
    }

    #[test]
    fn member_assignment_skips_used_addresses() {
        let used = vec![addr("42:1/100"), addr("42:1/101"), addr("42:1/900")];
        let next = next_member_address(&used).expect("next address");

        assert_eq!(next.to_string(), "42:1/102");
    }

    #[test]
    fn address_helpers_reject_points_for_top_level_assignment() {
        assert!(is_assignable_member_address(&addr("42:1/100")));
        assert!(!is_assignable_member_address(&addr("42:1/100.1")));
        assert!(is_test_lab_address(&addr("42:1/900")));
        assert!(!is_test_lab_address(&addr("42:1/900.1")));
    }

    #[test]
    fn config_package_validates_prd_shape() {
        valid_package().validate().expect("valid package");
    }

    #[test]
    fn config_package_rejects_wrong_network_key() {
        let mut package = valid_package();
        package.oxidenet.network.key = "fidonet".to_string();

        let error = package.validate().expect_err("invalid package");

        assert!(error.to_string().contains("network key"));
    }

    #[test]
    fn config_package_rejects_credential_mismatch() {
        let mut package = valid_package();
        package.credentials.session_password = "different".to_string();

        let error = package.validate().expect_err("invalid package");

        assert!(error.to_string().contains("session_password must match"));
    }

    #[test]
    fn area_tags_are_uppercase_ascii() {
        validate_area_tag("OXIDE.GENERAL").expect("valid tag");
        assert!(validate_area_tag("oxide.general").is_err());
        assert!(validate_area_tag("OXIDE GENERAL").is_err());
        assert!(validate_area_tag("").is_err());
    }
}
