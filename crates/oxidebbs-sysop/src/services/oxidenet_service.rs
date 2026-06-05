use std::collections::BTreeSet;

use oxidebbs_db::{
    NetworkAreaRecord, NetworkLinkRecord, NetworkNodelistRecord, NetworkPacketRecord,
    NetworkPollLogRecord, NetworkSubscriptionRecord, OxideDb, OxideNetApplicationRecord,
    OxideNetNodeRecord, find_network_profile_by_key, find_oxidenet_node_by_address,
    list_network_areas, list_network_links, list_network_nodelist_entries, list_network_packets,
    list_network_poll_logs, list_network_subscriptions, list_oxidenet_applications,
    list_oxidenet_credentials_for_node, list_oxidenet_nodes,
};
use oxidebbs_oxidenet::{
    DEFAULT_MAX_ACTIVE_JOIN_TOKENS, HubSettings, OXIDENET_NETWORK_KEY, OxideNetAdmin,
    ReviewDecision,
};

use crate::SysopError;
use crate::services::audit_service::AuditService;

#[derive(Debug, Clone, PartialEq)]
pub struct OxideNetDashboard {
    pub applications: Vec<OxideNetApplicationRecord>,
    pub nodes: Vec<OxideNetNodeRecord>,
    pub links: Vec<NetworkLinkRecord>,
    pub areas: Vec<NetworkAreaRecord>,
    pub packets: Vec<NetworkPacketRecord>,
    pub subscription_rows: Vec<NetworkSubscriptionRecord>,
    pub poll_log_rows: Vec<NetworkPollLogRecord>,
    pub nodelist: Vec<NetworkNodelistRecord>,
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
        let profile = find_network_profile_by_key(db.db(), OXIDENET_NETWORK_KEY)?;
        let profile_id = profile.as_ref().map(|profile| profile.id.as_str());
        let links = list_network_links(db.db())?
            .into_iter()
            .filter(|link| profile_id == Some(link.network_id.as_str()))
            .collect::<Vec<_>>();
        let link_ids = links
            .iter()
            .map(|link| link.id.as_str())
            .collect::<BTreeSet<_>>();
        let areas = list_network_areas(db.db())?
            .into_iter()
            .filter(|area| profile_id == Some(area.network_id.as_str()))
            .collect::<Vec<_>>();
        let area_ids = areas
            .iter()
            .map(|area| area.id.as_str())
            .collect::<BTreeSet<_>>();
        let packets = list_network_packets(db.db())?
            .into_iter()
            .filter(|packet| profile_id == Some(packet.network_id.as_str()))
            .collect::<Vec<_>>();
        let subscriptions = list_network_subscriptions(db.db())?
            .into_iter()
            .filter(|subscription| {
                area_ids.contains(subscription.area_id.as_str())
                    || link_ids.contains(subscription.link_id.as_str())
            })
            .collect::<Vec<_>>();
        let poll_logs = list_network_poll_logs(db.db())?
            .into_iter()
            .filter(|poll| link_ids.contains(poll.link_id.as_str()))
            .collect::<Vec<_>>();
        let nodelist = list_network_nodelist_entries(db.db())?
            .into_iter()
            .filter(|entry| profile_id == Some(entry.network_id.as_str()))
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
            links,
            areas,
            subscription_rows: subscriptions.clone(),
            poll_log_rows: poll_logs.clone(),
            nodelist,
            active_tokens,
            packet_queue_count: packets
                .iter()
                .filter(|packet| packet.status == "pending")
                .count(),
            quarantine_count: packets
                .iter()
                .filter(|packet| packet.status == "quarantined")
                .count(),
            subscriptions: subscriptions.len(),
            poll_logs: poll_logs.len(),
            packets,
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

#[cfg(test)]
mod tests {
    use super::OxideNetAdminService;
    use oxidebbs_db::{NetworkPacketRecord, OxideDb, insert_network_packet};
    use oxidebbs_oxidenet::{HubSettings, OXIDENET_NETWORK_KEY, OxideNetAdmin};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn load_counts_only_oxidenet_profile_packets() {
        let db_path = temp_db_path("oxidenet-dashboard");
        let db = OxideDb::open_or_create(&db_path).expect("open db");
        OxideNetAdmin::install_default_hub(db.db(), &HubSettings::default()).expect("install hub");
        let profile = oxidebbs_db::find_network_profile_by_key(db.db(), OXIDENET_NETWORK_KEY)
            .expect("find profile")
            .expect("profile exists");
        insert_network_packet(
            db.db(),
            &NetworkPacketRecord {
                id: "00000000-0000-4000-8000-000000000651".to_string(),
                network_id: profile.id.clone(),
                direction: "inbound".to_string(),
                link_id: None,
                filename: "packet.pkt".to_string(),
                sha256: "a".repeat(64),
                size_bytes: 128,
                status: "pending".to_string(),
                error_message: None,
                received_at: None,
                processed_at: None,
                created_at: "2026-06-04T00:00:00.000000Z".to_string(),
            },
        )
        .expect("insert packet");

        let dashboard = OxideNetAdminService::load(&db).expect("load dashboard");

        assert_eq!(dashboard.packet_queue_count, 1);
        assert_eq!(dashboard.packets.len(), 1);
        assert_eq!(
            dashboard.areas.len(),
            oxidebbs_oxidenet::DEFAULT_AREAS.len()
        );
        let _ = std::fs::remove_file(db_path);
    }

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "oxidebbs-sysop-{name}-{}-{nanos}.ddb",
            std::process::id()
        ))
    }
}
