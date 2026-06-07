use sha2::{Digest, Sha256};
use thiserror::Error;

use oxidebbs_network::{FtnAddress, NetworkAddressError};

pub const OXIDENET_NETWORK_KEY: &str = "oxidenet";
pub const OXIDENET_NETWORK_NAME: &str = "OxideNet";
pub const OXIDENET_POLICY_VERSION: &str = "1.0";
pub const OXIDENET_PRIMARY_HUB_ADDRESS: &str = "42:1/1";
pub const OXIDENET_BACKUP_HUB_ADDRESS: &str = "42:1/2";
pub const OXIDENET_HUB_LINK_KEY: &str = "oxidenet-hub";
pub const OXIDENET_SESSION_CREDENTIAL_KIND: &str = "binkp_session";
pub const OXIDENET_INVITE_TOKEN_KIND: &str = "invite_token";
pub const DEFAULT_MAX_ACTIVE_JOIN_TOKENS: usize = 8;
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

    #[error("database error: {0}")]
    Database(#[from] oxidebbs_db::DbError),
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ConfigPackage {
    pub oxidenet: OxideNetToml,
    pub areas: AreasToml,
    pub nodelist: NodelistToml,
    pub credentials: CredentialsToml,
    pub generated_at: String,
    pub token_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct OxideNetToml {
    pub network: PackageNetwork,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PackageLocal {
    pub board_name: String,
    pub sysop_alias: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PackageHub {
    pub name: String,
    pub host: String,
    pub binkp_port: u16,
    pub poll_interval_minutes: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PackageAuth {
    pub session_password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PackagePolicy {
    pub accepted_policy_version: String,
    pub accepted_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct AreasToml {
    pub areas: Vec<PackageArea>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PackageArea {
    pub tag: String,
    pub local_key: String,
    pub name: String,
    pub subscribed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct NodelistToml {
    pub nodes: Vec<PackageNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PackageNode {
    pub address: String,
    pub board_name: String,
    pub sysop_alias: String,
    pub host: String,
    pub binkp_port: u16,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CredentialsToml {
    pub address: String,
    pub hub_address: String,
    pub session_password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicationSubmission {
    pub applicant_user_id: Option<String>,
    pub board_name: String,
    pub sysop_alias: String,
    pub contact_email: String,
    pub host: String,
    pub binkp_port: u16,
    pub telnet_host: Option<String>,
    pub telnet_port: Option<u16>,
    pub software: String,
    pub software_version: String,
    pub timezone: String,
    pub region: String,
    pub description: String,
    pub reason: String,
    pub policy_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubSettings {
    pub board_name: String,
    pub sysop_alias: String,
    pub host: String,
    pub binkp_port: u16,
    pub poll_interval_minutes: u16,
    pub policy_version: String,
}

impl Default for HubSettings {
    fn default() -> Self {
        Self {
            board_name: "Blackboard BBS".to_string(),
            sysop_alias: "Blackboard Sysop".to_string(),
            host: "blackboard.example.net".to_string(),
            binkp_port: 24554,
            poll_interval_minutes: DEFAULT_POLL_INTERVAL_MINUTES,
            policy_version: OXIDENET_POLICY_VERSION.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApprovalOutcome {
    pub application: oxidebbs_db::OxideNetApplicationRecord,
    pub node: oxidebbs_db::OxideNetNodeRecord,
    pub credential: oxidebbs_db::OxideNetCredentialRecord,
    pub session_password: String,
    pub config_package: ConfigPackage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinToken {
    pub credential: oxidebbs_db::OxideNetCredentialRecord,
    pub plaintext: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigImportReport {
    pub profile_created: bool,
    pub link_created: bool,
    pub local_areas_created: usize,
    pub network_areas_created: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefaultHubReport {
    pub profile_created: bool,
    pub hub_node_created: bool,
    pub local_areas_created: usize,
    pub network_areas_created: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewDecision {
    Reject,
    RequestInfo,
    Hold,
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

impl ApplicationSubmission {
    pub fn validate(&self) -> Result<(), OxideNetError> {
        require_nonblank("board_name", &self.board_name)?;
        require_nonblank("sysop_alias", &self.sysop_alias)?;
        require_nonblank("contact_email", &self.contact_email)?;
        require_nonblank("host", &self.host)?;
        require_nonblank("software", &self.software)?;
        require_nonblank("software_version", &self.software_version)?;
        require_nonblank("timezone", &self.timezone)?;
        require_nonblank("region", &self.region)?;
        require_nonblank("description", &self.description)?;
        require_nonblank("reason", &self.reason)?;
        require_nonblank("policy_version", &self.policy_version)?;
        if self.binkp_port == 0 {
            return Err(OxideNetError::Network(
                "application BinkP port must be non-zero".to_string(),
            ));
        }
        if self.telnet_port == Some(0) {
            return Err(OxideNetError::Network(
                "application telnet port must be non-zero when present".to_string(),
            ));
        }
        Ok(())
    }
}

pub struct OxideNetAdmin;

impl OxideNetAdmin {
    pub fn submit_application(
        db: &oxidebbs_db::Db,
        submission: &ApplicationSubmission,
    ) -> Result<oxidebbs_db::OxideNetApplicationRecord, OxideNetError> {
        submission.validate()?;
        let now = current_timestamp(db)?;
        let record = oxidebbs_db::OxideNetApplicationRecord {
            id: generated_uuid(db)?,
            created_at: now.clone(),
            updated_at: now.clone(),
            submitted_at: Some(now.clone()),
            reviewed_at: None,
            status: "submitted".to_string(),
            applicant_user_id: submission.applicant_user_id.clone(),
            board_name: submission.board_name.clone(),
            sysop_alias: submission.sysop_alias.clone(),
            contact_email: submission.contact_email.clone(),
            host: submission.host.clone(),
            binkp_port: i64::from(submission.binkp_port),
            telnet_host: submission.telnet_host.clone(),
            telnet_port: submission.telnet_port.map(i64::from),
            software: submission.software.clone(),
            software_version: submission.software_version.clone(),
            timezone: submission.timezone.clone(),
            region: submission.region.clone(),
            description: submission.description.clone(),
            reason: submission.reason.clone(),
            policy_version: submission.policy_version.clone(),
            policy_accepted_at: Some(now),
            admin_notes: String::new(),
            reviewed_by_user_id: None,
            assigned_address: None,
        };
        oxidebbs_db::insert_oxidenet_application(db, &record)?;
        Ok(record)
    }

    pub fn review_application(
        db: &oxidebbs_db::Db,
        application_id: &str,
        decision: ReviewDecision,
        reviewed_by_user_id: Option<&str>,
        admin_notes: Option<&str>,
    ) -> Result<oxidebbs_db::OxideNetApplicationRecord, OxideNetError> {
        let status = match decision {
            ReviewDecision::Reject => "rejected",
            ReviewDecision::RequestInfo => "needs-info",
            ReviewDecision::Hold => "hold",
        };
        let now = current_timestamp(db)?;
        let updated = oxidebbs_db::update_oxidenet_application_status(
            db,
            application_id,
            status,
            Some(&now),
            reviewed_by_user_id,
            admin_notes,
            None,
        )?;
        if !updated {
            return Err(OxideNetError::Network(format!(
                "application {application_id} was not found"
            )));
        }
        require_application(db, application_id)
    }

    pub fn approve_application(
        db: &oxidebbs_db::Db,
        application_id: &str,
        reviewed_by_user_id: Option<&str>,
        assigned_address: Option<&str>,
        admin_notes: Option<&str>,
        hub: &HubSettings,
    ) -> Result<ApprovalOutcome, OxideNetError> {
        let application = require_application(db, application_id)?;
        let address = match assigned_address {
            Some(address) => parse_oxidenet_address(address)?,
            None => {
                let used = oxidebbs_db::list_oxidenet_nodes(db, 10_000)?
                    .iter()
                    .filter_map(|node| node.address.parse::<FtnAddress>().ok())
                    .collect::<Vec<_>>();
                next_member_address(used.iter())?
            }
        };
        if !is_assignable_member_address(&address) && !is_test_lab_address(&address) {
            return Err(address_error(
                &address,
                "approved application address must be a member or test-lab address",
            ));
        }
        if oxidebbs_db::find_oxidenet_node_by_address(db, &address.to_string())?.is_some() {
            return Err(OxideNetError::DuplicateApplication(address.to_string()));
        }

        let now = current_timestamp(db)?;
        let session_password = generated_secret(db, "session")?;
        let node = oxidebbs_db::OxideNetNodeRecord {
            id: generated_uuid(db)?,
            application_id: Some(application.id.clone()),
            network_key: OXIDENET_NETWORK_KEY.to_string(),
            address: address.to_string(),
            zone: i64::from(address.zone),
            net: i64::from(address.net),
            node: i64::from(address.node),
            point: i64::from(address.point.unwrap_or(0)),
            hub_address: OXIDENET_PRIMARY_HUB_ADDRESS.to_string(),
            board_name: application.board_name.clone(),
            sysop_alias: application.sysop_alias.clone(),
            contact_email: application.contact_email.clone(),
            host: application.host.clone(),
            binkp_port: application.binkp_port,
            telnet_host: application.telnet_host.clone(),
            telnet_port: application.telnet_port,
            software: application.software.clone(),
            software_version: application.software_version.clone(),
            status: "first-poll-pending".to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
            activated_at: None,
            suspended_at: None,
            retired_at: None,
            last_poll_at: None,
            last_successful_poll_at: None,
            flags: "CM,TLS".to_string(),
        };
        oxidebbs_db::insert_oxidenet_node(db, &node)?;

        let credential = oxidebbs_db::OxideNetCredentialRecord {
            id: generated_uuid(db)?,
            node_id: node.id.clone(),
            credential_kind: OXIDENET_SESSION_CREDENTIAL_KIND.to_string(),
            secret_hash: hash_secret(&session_password),
            created_at: now.clone(),
            rotated_at: None,
            expires_at: None,
            status: "active".to_string(),
        };
        oxidebbs_db::insert_oxidenet_credential(db, &credential)?;

        oxidebbs_db::update_oxidenet_application_status(
            db,
            application_id,
            "approved",
            Some(&now),
            reviewed_by_user_id,
            admin_notes,
            Some(&address.to_string()),
        )?;
        let updated_application = require_application(db, application_id)?;
        let config_package =
            Self::config_package_for_node(db, &node, hub, &session_password, &now)?;

        Ok(ApprovalOutcome {
            application: updated_application,
            node,
            credential,
            session_password,
            config_package,
        })
    }

    pub fn issue_join_token(
        db: &oxidebbs_db::Db,
        node_id: &str,
        max_active: usize,
    ) -> Result<JoinToken, OxideNetError> {
        let credentials = oxidebbs_db::list_oxidenet_credentials_for_node(db, node_id)?;
        let active_tokens = credentials
            .iter()
            .filter(|credential| {
                credential.credential_kind == OXIDENET_INVITE_TOKEN_KIND
                    && credential.status == "active"
            })
            .count();
        if active_tokens >= max_active {
            return Err(OxideNetError::Network(format!(
                "node {node_id} already has {active_tokens} active join token(s); limit is {max_active}"
            )));
        }
        let now = current_timestamp(db)?;
        let plaintext = generated_secret(db, "join")?;
        let credential = oxidebbs_db::OxideNetCredentialRecord {
            id: generated_uuid(db)?,
            node_id: node_id.to_string(),
            credential_kind: OXIDENET_INVITE_TOKEN_KIND.to_string(),
            secret_hash: hash_secret(&plaintext),
            created_at: now,
            rotated_at: None,
            expires_at: None,
            status: "active".to_string(),
        };
        oxidebbs_db::insert_oxidenet_credential(db, &credential)?;
        Ok(JoinToken {
            credential,
            plaintext,
        })
    }

    pub fn revoke_token(db: &oxidebbs_db::Db, credential_id: &str) -> Result<bool, OxideNetError> {
        Ok(oxidebbs_db::revoke_oxidenet_credential(
            db,
            credential_id,
            &current_timestamp(db)?,
        )?)
    }

    pub fn rotate_session_password(
        db: &oxidebbs_db::Db,
        node_id: &str,
    ) -> Result<JoinToken, OxideNetError> {
        let now = current_timestamp(db)?;
        for credential in oxidebbs_db::list_oxidenet_credentials_for_node(db, node_id)?
            .into_iter()
            .filter(|credential| {
                credential.credential_kind == OXIDENET_SESSION_CREDENTIAL_KIND
                    && credential.status == "active"
            })
        {
            oxidebbs_db::revoke_oxidenet_credential(db, &credential.id, &now)?;
        }

        let plaintext = generated_secret(db, "session")?;
        let credential = oxidebbs_db::OxideNetCredentialRecord {
            id: generated_uuid(db)?,
            node_id: node_id.to_string(),
            credential_kind: OXIDENET_SESSION_CREDENTIAL_KIND.to_string(),
            secret_hash: hash_secret(&plaintext),
            created_at: now,
            rotated_at: None,
            expires_at: None,
            status: "active".to_string(),
        };
        oxidebbs_db::insert_oxidenet_credential(db, &credential)?;
        Ok(JoinToken {
            credential,
            plaintext,
        })
    }

    pub fn set_node_suspended(
        db: &oxidebbs_db::Db,
        node_id: &str,
        suspended: bool,
    ) -> Result<bool, OxideNetError> {
        let status = if suspended { "suspended" } else { "active" };
        Ok(oxidebbs_db::update_oxidenet_node_status(
            db,
            node_id,
            status,
            &current_timestamp(db)?,
        )?)
    }

    pub fn record_node_poll(
        db: &oxidebbs_db::Db,
        address: &str,
        successful: bool,
    ) -> Result<bool, OxideNetError> {
        let Some(node) = oxidebbs_db::find_oxidenet_node_by_address(db, address)? else {
            return Ok(false);
        };
        let updated = oxidebbs_db::record_oxidenet_node_poll(
            db,
            &node.id,
            &current_timestamp(db)?,
            successful,
        )?;
        if successful && node.status == "first-poll-pending" {
            let _ = oxidebbs_db::update_oxidenet_node_status(
                db,
                &node.id,
                "active",
                &current_timestamp(db)?,
            )?;
        }
        Ok(updated)
    }

    pub fn ensure_node_can_exchange(
        db: &oxidebbs_db::Db,
        address: &str,
    ) -> Result<(), OxideNetError> {
        if let Some(node) = oxidebbs_db::find_oxidenet_node_by_address(db, address)?
            && node.status == "suspended"
        {
            return Err(OxideNetError::Network(format!(
                "OxideNet node {address} is suspended"
            )));
        }
        Ok(())
    }

    pub fn config_package_for_node(
        db: &oxidebbs_db::Db,
        node: &oxidebbs_db::OxideNetNodeRecord,
        hub: &HubSettings,
        session_password: &str,
        generated_at: &str,
    ) -> Result<ConfigPackage, OxideNetError> {
        let nodes = oxidebbs_db::list_oxidenet_nodes(db, 10_000)?;
        let mut package_nodes = vec![PackageNode {
            address: OXIDENET_PRIMARY_HUB_ADDRESS.to_string(),
            board_name: hub.board_name.clone(),
            sysop_alias: hub.sysop_alias.clone(),
            host: hub.host.clone(),
            binkp_port: hub.binkp_port,
            status: "active".to_string(),
        }];
        package_nodes.extend(nodes.into_iter().map(|node| PackageNode {
            address: node.address,
            board_name: node.board_name,
            sysop_alias: node.sysop_alias,
            host: node.host,
            binkp_port: u16::try_from(node.binkp_port).unwrap_or(24554),
            status: node.status,
        }));
        package_nodes.sort_by(|left, right| left.address.cmp(&right.address));
        package_nodes.dedup_by(|left, right| left.address == right.address);

        let package = ConfigPackage {
            oxidenet: OxideNetToml {
                network: PackageNetwork {
                    key: OXIDENET_NETWORK_KEY.to_string(),
                    name: OXIDENET_NETWORK_NAME.to_string(),
                    address: node.address.clone(),
                    hub_address: OXIDENET_PRIMARY_HUB_ADDRESS.to_string(),
                    local: PackageLocal {
                        board_name: node.board_name.clone(),
                        sysop_alias: node.sysop_alias.clone(),
                    },
                    hub: PackageHub {
                        name: hub.board_name.clone(),
                        host: hub.host.clone(),
                        binkp_port: hub.binkp_port,
                        poll_interval_minutes: hub.poll_interval_minutes,
                    },
                    auth: PackageAuth {
                        session_password: session_password.to_string(),
                    },
                    policy: PackagePolicy {
                        accepted_policy_version: hub.policy_version.clone(),
                        accepted_at: generated_at.to_string(),
                    },
                },
            },
            areas: AreasToml {
                areas: default_package_areas(),
            },
            nodelist: NodelistToml {
                nodes: package_nodes,
            },
            credentials: CredentialsToml {
                address: node.address.clone(),
                hub_address: OXIDENET_PRIMARY_HUB_ADDRESS.to_string(),
                session_password: session_password.to_string(),
            },
            generated_at: generated_at.to_string(),
            token_hash: hash_secret(session_password),
        };
        package.validate()?;
        Ok(package)
    }

    pub fn import_config_package(
        db: &oxidebbs_db::Db,
        package: &ConfigPackage,
    ) -> Result<ConfigImportReport, OxideNetError> {
        package.validate()?;
        let network = &package.oxidenet.network;
        let address = parse_oxidenet_address(&network.address)?;
        let now = current_timestamp(db)?;
        let mut report = ConfigImportReport {
            profile_created: false,
            link_created: false,
            local_areas_created: 0,
            network_areas_created: 0,
        };

        let profile = match oxidebbs_db::find_network_profile_by_key(db, OXIDENET_NETWORK_KEY)? {
            Some(profile) => profile,
            None => {
                let profile = oxidebbs_db::NetworkProfileRecord {
                    id: generated_uuid(db)?,
                    key: OXIDENET_NETWORK_KEY.to_string(),
                    name: OXIDENET_NETWORK_NAME.to_string(),
                    adapter: OXIDENET_NETWORK_KEY.to_string(),
                    local_zone: i64::from(address.zone),
                    local_net: i64::from(address.net),
                    local_node: i64::from(address.node),
                    local_point: i64::from(address.point.unwrap_or(0)),
                    enabled: true,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                };
                oxidebbs_db::insert_network_profile(db, &profile)?;
                report.profile_created = true;
                profile
            }
        };

        if oxidebbs_db::find_network_link_by_key(db, OXIDENET_HUB_LINK_KEY)?.is_none() {
            oxidebbs_db::insert_network_link(
                db,
                &oxidebbs_db::NetworkLinkRecord {
                    id: generated_uuid(db)?,
                    key: OXIDENET_HUB_LINK_KEY.to_string(),
                    network_id: profile.id.clone(),
                    address: network.hub_address.clone(),
                    host: network.hub.host.clone(),
                    binkp_port: i64::from(network.hub.binkp_port),
                    password: network.auth.session_password.clone(),
                    poll_schedule_minutes: i64::from(network.hub.poll_interval_minutes),
                    compression: "zip".to_string(),
                    transport_security: "tls_required".to_string(),
                    enabled: true,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                },
            )?;
            report.link_created = true;
        }

        for area in &package.areas.areas {
            let local_area = match oxidebbs_db::find_message_area_by_key(db, &area.local_key)? {
                Some(existing) => existing,
                None => {
                    let local_area = oxidebbs_db::MessageAreaRecord {
                        id: generated_uuid(db)?,
                        key: area.local_key.clone(),
                        name: area.name.clone(),
                        description: format!("OxideNet area {}", area.tag),
                        kind: "echomail".to_string(),
                        network_id: Some(profile.id.clone()),
                        read_security_level: 10,
                        post_security_level: 10,
                        moderated: false,
                        enabled: true,
                    };
                    oxidebbs_db::insert_message_area(db, &local_area)?;
                    report.local_areas_created += 1;
                    local_area
                }
            };
            if oxidebbs_db::find_network_area_by_tag_and_profile(db, &profile.id, &area.tag)?
                .is_none()
            {
                oxidebbs_db::insert_network_area(
                    db,
                    &oxidebbs_db::NetworkAreaRecord {
                        id: generated_uuid(db)?,
                        network_id: profile.id.clone(),
                        area_tag: area.tag.clone(),
                        local_area_id: local_area.id,
                        description: area.name.clone(),
                        read_only: false,
                        subscribed: area.subscribed,
                        created_at: now.clone(),
                        updated_at: now.clone(),
                    },
                )?;
                report.network_areas_created += 1;
            }
        }

        Ok(report)
    }

    pub fn install_default_hub(
        db: &oxidebbs_db::Db,
        hub: &HubSettings,
    ) -> Result<DefaultHubReport, OxideNetError> {
        let now = current_timestamp(db)?;
        let mut report = DefaultHubReport {
            profile_created: false,
            hub_node_created: false,
            local_areas_created: 0,
            network_areas_created: 0,
        };
        let profile = match oxidebbs_db::find_network_profile_by_key(db, OXIDENET_NETWORK_KEY)? {
            Some(profile) => profile,
            None => {
                let profile = oxidebbs_db::NetworkProfileRecord {
                    id: generated_uuid(db)?,
                    key: OXIDENET_NETWORK_KEY.to_string(),
                    name: OXIDENET_NETWORK_NAME.to_string(),
                    adapter: OXIDENET_NETWORK_KEY.to_string(),
                    local_zone: i64::from(OXIDENET_ZONE_U16),
                    local_net: i64::from(OXIDENET_PRIMARY_NET),
                    local_node: i64::from(DEFAULT_HUB_NODE),
                    local_point: 0,
                    enabled: true,
                    created_at: now.clone(),
                    updated_at: now.clone(),
                };
                oxidebbs_db::insert_network_profile(db, &profile)?;
                report.profile_created = true;
                profile
            }
        };

        if oxidebbs_db::find_oxidenet_node_by_address(db, OXIDENET_PRIMARY_HUB_ADDRESS)?.is_none() {
            let hub_node_id = generated_uuid(db)?;
            oxidebbs_db::insert_oxidenet_node(
                db,
                &oxidebbs_db::OxideNetNodeRecord {
                    id: hub_node_id.clone(),
                    application_id: None,
                    network_key: OXIDENET_NETWORK_KEY.to_string(),
                    address: OXIDENET_PRIMARY_HUB_ADDRESS.to_string(),
                    zone: i64::from(OXIDENET_ZONE_U16),
                    net: i64::from(OXIDENET_PRIMARY_NET),
                    node: i64::from(DEFAULT_HUB_NODE),
                    point: 0,
                    hub_address: OXIDENET_PRIMARY_HUB_ADDRESS.to_string(),
                    board_name: hub.board_name.clone(),
                    sysop_alias: hub.sysop_alias.clone(),
                    contact_email: "sysop@blackboard.example.net".to_string(),
                    host: hub.host.clone(),
                    binkp_port: i64::from(hub.binkp_port),
                    telnet_host: None,
                    telnet_port: None,
                    software: "OxideBBS".to_string(),
                    software_version: env!("CARGO_PKG_VERSION").to_string(),
                    status: "first-poll-pending".to_string(),
                    created_at: now.clone(),
                    updated_at: now.clone(),
                    activated_at: None,
                    suspended_at: None,
                    retired_at: None,
                    last_poll_at: None,
                    last_successful_poll_at: None,
                    flags: "CM,HUB,TLS".to_string(),
                },
            )?;
            oxidebbs_db::update_oxidenet_node_status(db, &hub_node_id, "active", &now)?;
            report.hub_node_created = true;
        }

        for area in default_package_areas() {
            let local_area = match oxidebbs_db::find_message_area_by_key(db, &area.local_key)? {
                Some(existing) => existing,
                None => {
                    let local_area = oxidebbs_db::MessageAreaRecord {
                        id: generated_uuid(db)?,
                        key: area.local_key.clone(),
                        name: area.name.clone(),
                        description: format!("OxideNet hub area {}", area.tag),
                        kind: "echomail".to_string(),
                        network_id: Some(profile.id.clone()),
                        read_security_level: 10,
                        post_security_level: 10,
                        moderated: false,
                        enabled: true,
                    };
                    oxidebbs_db::insert_message_area(db, &local_area)?;
                    report.local_areas_created += 1;
                    local_area
                }
            };
            if oxidebbs_db::find_network_area_by_tag_and_profile(db, &profile.id, &area.tag)?
                .is_none()
            {
                oxidebbs_db::insert_network_area(
                    db,
                    &oxidebbs_db::NetworkAreaRecord {
                        id: generated_uuid(db)?,
                        network_id: profile.id.clone(),
                        area_tag: area.tag.clone(),
                        local_area_id: local_area.id,
                        description: area.name,
                        read_only: false,
                        subscribed: true,
                        created_at: now.clone(),
                        updated_at: now.clone(),
                    },
                )?;
                report.network_areas_created += 1;
            }
        }

        Ok(report)
    }

    pub fn generate_nodelist(
        db: &oxidebbs_db::Db,
    ) -> Result<Vec<oxidebbs_db::NetworkNodelistRecord>, OxideNetError> {
        let profile = oxidebbs_db::find_network_profile_by_key(db, OXIDENET_NETWORK_KEY)?
            .ok_or_else(|| {
                OxideNetError::Network(
                    "OxideNet profile is not installed; run net oxidenet install-hub first"
                        .to_string(),
                )
            })?;
        let now = current_timestamp(db)?;
        let mut records = Vec::new();
        for node in oxidebbs_db::list_oxidenet_nodes(db, 10_000)?
            .into_iter()
            .filter(|node| node.status != "retired")
        {
            let address = parse_oxidenet_address(&node.address)?;
            let raw_entry = format!(
                "Node,{},{},{},{},{}",
                node.address,
                sanitize_nodelist_text(&node.board_name),
                sanitize_nodelist_text(&node.sysop_alias),
                node.binkp_port,
                node.flags
            );
            records.push(oxidebbs_db::NetworkNodelistRecord {
                id: generated_uuid(db)?,
                network_id: profile.id.clone(),
                zone: i64::from(address.zone),
                net: i64::from(address.net),
                node: i64::from(address.node),
                point: i64::from(address.point.unwrap_or(0)),
                parsed_name: Some(node.board_name),
                location: Some("OxideNet".to_string()),
                sysop_name: Some(node.sysop_alias),
                phone: Some(node.host),
                speed: Some(node.binkp_port.to_string()),
                flags: node.flags,
                raw_entry,
                updated_at: now.clone(),
            });
        }
        records.sort_by_key(|record| (record.zone, record.net, record.node, record.point));
        oxidebbs_db::replace_network_nodelist_entries(db, &profile.id, &records)?;
        Ok(records)
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

#[must_use]
pub fn default_package_areas() -> Vec<PackageArea> {
    DEFAULT_AREAS
        .iter()
        .map(|tag| PackageArea {
            tag: (*tag).to_string(),
            local_key: tag.to_ascii_lowercase().replace('.', "-"),
            name: format!("OxideNet {tag}"),
            subscribed: true,
        })
        .collect()
}

#[must_use]
pub fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn generated_secret(db: &oxidebbs_db::Db, prefix: &str) -> Result<String, OxideNetError> {
    let seed = generated_uuid(db)?;
    let mut hasher = Sha256::new();
    hasher.update(prefix.as_bytes());
    hasher.update(b":");
    hasher.update(seed.as_bytes());
    Ok(format!(
        "oxide-{prefix}-{}",
        &hex::encode(hasher.finalize())[..32]
    ))
}

fn generated_uuid(db: &oxidebbs_db::Db) -> Result<String, OxideNetError> {
    db_scalar_text(db, "SELECT UUID_TO_STRING(GEN_RANDOM_UUID())")
}

fn current_timestamp(db: &oxidebbs_db::Db) -> Result<String, OxideNetError> {
    db_scalar_text(db, "SELECT CAST(NOW() AS TEXT)")
}

fn db_scalar_text(db: &oxidebbs_db::Db, sql: &str) -> Result<String, OxideNetError> {
    let result = db.execute(sql)?;
    let value = result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .ok_or_else(|| OxideNetError::Network(format!("query returned no rows: {sql}")))?;
    match value {
        oxidebbs_db::Value::Text(value) => Ok(value.clone()),
        other => Err(OxideNetError::Network(format!(
            "query returned non-text scalar for {sql}: {other:?}"
        ))),
    }
}

fn require_application(
    db: &oxidebbs_db::Db,
    application_id: &str,
) -> Result<oxidebbs_db::OxideNetApplicationRecord, OxideNetError> {
    oxidebbs_db::find_oxidenet_application_by_id(db, application_id)?.ok_or_else(|| {
        OxideNetError::Network(format!("application {application_id} was not found"))
    })
}

fn require_nonblank(field: &str, value: &str) -> Result<(), OxideNetError> {
    if value.trim().is_empty() {
        return Err(OxideNetError::Network(format!("{field} is required")));
    }
    Ok(())
}

fn sanitize_nodelist_text(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            ',' | '\r' | '\n' | '\t' => '_',
            other => other,
        })
        .collect()
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

    fn test_db() -> oxidebbs_db::OxideDb {
        oxidebbs_db::OxideDb::open_memory().expect("open test db")
    }

    fn application_submission() -> ApplicationSubmission {
        ApplicationSubmission {
            applicant_user_id: None,
            board_name: "Retro Cavern BBS".to_string(),
            sysop_alias: "Night Owl".to_string(),
            contact_email: "sysop@example.test".to_string(),
            host: "retro.example.test".to_string(),
            binkp_port: 24554,
            telnet_host: Some("retro.example.test".to_string()),
            telnet_port: Some(23),
            software: "OxideBBS".to_string(),
            software_version: "1.2.2".to_string(),
            timezone: "America/Chicago".to_string(),
            region: "NA".to_string(),
            description: "A retro ANSI board focused on doors and echomail.".to_string(),
            reason: "Join the public OxideNet experiment.".to_string(),
            policy_version: OXIDENET_POLICY_VERSION.to_string(),
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

    #[test]
    fn hub_member_application_package_and_nodelist_flow() {
        let hub_db = test_db();
        let hub = HubSettings::default();
        let install = OxideNetAdmin::install_default_hub(hub_db.db(), &hub).expect("install hub");
        assert!(install.profile_created);
        assert!(install.hub_node_created);
        assert_eq!(install.network_areas_created, DEFAULT_AREAS.len());

        let application = OxideNetAdmin::submit_application(hub_db.db(), &application_submission())
            .expect("submit application");
        assert_eq!(application.status, "submitted");

        let approval = OxideNetAdmin::approve_application(
            hub_db.db(),
            &application.id,
            None,
            None,
            None,
            &hub,
        )
        .expect("approve application");
        assert_eq!(approval.application.status, "approved");
        assert_eq!(approval.node.address, "42:1/100");
        assert!(approval.session_password.starts_with("oxide-session-"));
        approval.config_package.validate().expect("valid package");

        let token = OxideNetAdmin::issue_join_token(
            hub_db.db(),
            &approval.node.id,
            DEFAULT_MAX_ACTIVE_JOIN_TOKENS,
        )
        .expect("issue token");
        assert!(token.plaintext.starts_with("oxide-join-"));
        assert_eq!(token.credential.credential_kind, OXIDENET_INVITE_TOKEN_KIND);
        assert!(OxideNetAdmin::revoke_token(hub_db.db(), &token.credential.id).expect("revoke"));

        let nodelist = OxideNetAdmin::generate_nodelist(hub_db.db()).expect("generate nodelist");
        assert_eq!(nodelist.len(), 2);
        assert!(nodelist.iter().any(|entry| entry.node == 100));

        let member_db = test_db();
        let import = OxideNetAdmin::import_config_package(member_db.db(), &approval.config_package)
            .expect("import package");
        assert!(import.profile_created);
        assert!(import.link_created);
        assert_eq!(import.network_areas_created, DEFAULT_AREAS.len());
    }

    #[test]
    fn suspended_nodes_are_rejected_for_exchange() {
        let db = test_db();
        let hub = HubSettings::default();
        OxideNetAdmin::install_default_hub(db.db(), &hub).expect("install hub");
        let application = OxideNetAdmin::submit_application(db.db(), &application_submission())
            .expect("submit application");
        let approval =
            OxideNetAdmin::approve_application(db.db(), &application.id, None, None, None, &hub)
                .expect("approve application");

        OxideNetAdmin::ensure_node_can_exchange(db.db(), &approval.node.address)
            .expect("active enough");
        assert!(
            OxideNetAdmin::set_node_suspended(db.db(), &approval.node.id, true).expect("suspend")
        );
        let error = OxideNetAdmin::ensure_node_can_exchange(db.db(), &approval.node.address)
            .expect_err("suspended node rejected");
        assert!(error.to_string().contains("suspended"));
    }
}
