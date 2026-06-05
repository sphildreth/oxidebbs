use oxidebbs_db::{
    OxideDb, OxideNetApplicationRecord, OxideNetNodeRecord, find_oxidenet_node_by_address,
    list_network_packets, list_network_poll_logs, list_network_subscriptions,
    list_oxidenet_applications, list_oxidenet_credentials_for_node, list_oxidenet_nodes,
};
use oxidebbs_oxidenet::{
    DEFAULT_MAX_ACTIVE_JOIN_TOKENS, HubSettings, OXIDENET_NETWORK_KEY, OxideNetAdmin,
    ReviewDecision,
};

use crate::SysopError;
use crate::services::audit_service::AuditService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OxideNetDashboard {
    pub applications: Vec<OxideNetApplicationRecord>,
    pub nodes: Vec<OxideNetNodeRecord>,
    pub pending_applications: usize,
    pub suspended_nodes: usize,
    pub active_tokens: usize,
    pub packet_queue_count: usize,
    pub quarantine_count: usize,
    pub subscriptions: usize,
    pub poll_logs: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretResult {
    pub credential_id: String,
    pub plaintext: String,
}

pub struct OxideNetAdminService;

impl OxideNetAdminService {
    pub fn load(db: &OxideDb) -> Result<OxideNetDashboard, SysopError> {
        let applications = list_oxidenet_applications(db.db(), 500)?;
        let nodes = list_oxidenet_nodes(db.db(), 500)?;
        let mut active_tokens = 0;
        for node in &nodes {
            active_tokens += list_oxidenet_credentials_for_node(db.db(), &node.id)?
                .into_iter()
                .filter(|credential| {
                    credential.credential_kind == oxidebbs_oxidenet::OXIDENET_INVITE_TOKEN_KIND
                        && credential.status == "active"
                })
                .count();
        }
        let packets = list_network_packets(db.db())?
            .into_iter()
            .filter(|packet| packet.network_id == OXIDENET_NETWORK_KEY)
            .collect::<Vec<_>>();

        Ok(OxideNetDashboard {
            pending_applications: applications
                .iter()
                .filter(|application| application.status == "submitted")
                .count(),
            suspended_nodes: nodes
                .iter()
                .filter(|node| node.status == "suspended")
                .count(),
            applications,
            nodes,
            active_tokens,
            packet_queue_count: packets
                .iter()
                .filter(|packet| packet.status == "pending")
                .count(),
            quarantine_count: packets
                .iter()
                .filter(|packet| packet.status == "quarantined")
                .count(),
            subscriptions: list_network_subscriptions(db.db())?.len(),
            poll_logs: list_network_poll_logs(db.db())?.len(),
        })
    }

    pub fn install_hub_defaults(db: &OxideDb) -> Result<(), SysopError> {
        let report = OxideNetAdmin::install_default_hub(db.db(), &HubSettings::default())
            .map_err(|error| SysopError::Message(error.to_string()))?;
        AuditService::record(
            db.db(),
            "oxidenet_install_hub",
            None,
            None,
            &format!(
                "profile_created={} hub_node_created={} local_areas_created={} network_areas_created={}",
                report.profile_created,
                report.hub_node_created,
                report.local_areas_created,
                report.network_areas_created
            ),
        )?;
        Ok(())
    }

    pub fn approve_application(
        db: &OxideDb,
        application_id: &str,
    ) -> Result<SecretResult, SysopError> {
        let outcome = OxideNetAdmin::approve_application(
            db.db(),
            application_id,
            None,
            None,
            Some("approved from sysop TUI"),
            &HubSettings::default(),
        )
        .map_err(|error| SysopError::Message(error.to_string()))?;
        AuditService::record(
            db.db(),
            "oxidenet_application_approved",
            None,
            None,
            &format!(
                "application_id={} node_id={} address={}",
                outcome.application.id, outcome.node.id, outcome.node.address
            ),
        )?;
        Ok(SecretResult {
            credential_id: outcome.credential.id,
            plaintext: outcome.session_password,
        })
    }

    pub fn review_application(
        db: &OxideDb,
        application_id: &str,
        decision: ReviewDecision,
    ) -> Result<(), SysopError> {
        let application = OxideNetAdmin::review_application(
            db.db(),
            application_id,
            decision,
            None,
            Some("reviewed from sysop TUI"),
        )
        .map_err(|error| SysopError::Message(error.to_string()))?;
        AuditService::record(
            db.db(),
            "oxidenet_application_reviewed",
            None,
            None,
            &format!(
                "application_id={} status={}",
                application.id, application.status
            ),
        )?;
        Ok(())
    }

    pub fn set_node_suspended(
        db: &OxideDb,
        node_id: &str,
        suspended: bool,
    ) -> Result<(), SysopError> {
        OxideNetAdmin::set_node_suspended(db.db(), node_id, suspended)
            .map_err(|error| SysopError::Message(error.to_string()))?;
        AuditService::record(
            db.db(),
            if suspended {
                "oxidenet_node_suspended"
            } else {
                "oxidenet_node_activated"
            },
            None,
            None,
            &format!("node_id={node_id}"),
        )?;
        Ok(())
    }

    pub fn rotate_node_password(db: &OxideDb, node_id: &str) -> Result<SecretResult, SysopError> {
        let token = OxideNetAdmin::rotate_session_password(db.db(), node_id)
            .map_err(|error| SysopError::Message(error.to_string()))?;
        AuditService::record(
            db.db(),
            "oxidenet_node_password_rotated",
            None,
            None,
            &format!("node_id={node_id} credential_id={}", token.credential.id),
        )?;
        Ok(SecretResult {
            credential_id: token.credential.id,
            plaintext: token.plaintext,
        })
    }

    pub fn issue_join_token(db: &OxideDb, node_id: &str) -> Result<SecretResult, SysopError> {
        let token =
            OxideNetAdmin::issue_join_token(db.db(), node_id, DEFAULT_MAX_ACTIVE_JOIN_TOKENS)
                .map_err(|error| SysopError::Message(error.to_string()))?;
        AuditService::record(
            db.db(),
            "oxidenet_join_token_issued",
            None,
            None,
            &format!("node_id={node_id} credential_id={}", token.credential.id),
        )?;
        Ok(SecretResult {
            credential_id: token.credential.id,
            plaintext: token.plaintext,
        })
    }

    pub fn generate_nodelist(db: &OxideDb) -> Result<usize, SysopError> {
        let records = OxideNetAdmin::generate_nodelist(db.db())
            .map_err(|error| SysopError::Message(error.to_string()))?;
        AuditService::record(
            db.db(),
            "oxidenet_nodelist_generated",
            None,
            None,
            &format!("entries={}", records.len()),
        )?;
        Ok(records.len())
    }

    pub fn node_by_id_or_address(
        db: &OxideDb,
        id_or_address: &str,
    ) -> Result<Option<OxideNetNodeRecord>, SysopError> {
        if let Some(node) = find_oxidenet_node_by_address(db.db(), id_or_address)? {
            return Ok(Some(node));
        }
        Ok(list_oxidenet_nodes(db.db(), 500)?
            .into_iter()
            .find(|node| node.id == id_or_address))
    }
}
