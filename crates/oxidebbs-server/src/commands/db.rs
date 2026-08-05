use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use serde::Deserialize;
use serde_json::Value as JsonValue;

use crate::{
    control::{ControlResponse, request_status},
    sysop_cli::{AppContext, CliError, CliResult, emit_ok, open_database, print_json},
};
use oxidebbs_db::{
    AuditEventRecord, AuthAttemptRecord, Db, DoorDefinitionRecord, DoorProviderCredentialRecord,
    DoorRunRecord, MessageAreaRecord, MessageRecord, NetworkAreaRecord, NetworkDuplicateLogRecord,
    NetworkLinkRecord, NetworkMessageRecord, NetworkNodelistRecord, NetworkPacketRecord,
    NetworkPathNode, NetworkPollLogRecord, NetworkProfileRecord, NetworkSeenByNode,
    NetworkSubscriptionRecord, OxideNetApplicationRecord, OxideNetCredentialRecord,
    OxideNetNodeRecord, SessionRecord, UserRecord, Value, evict_shared_wal,
    insert_audit_event_preserving_record, insert_auth_attempt, insert_door_definition,
    insert_door_provider_credential, insert_door_run, insert_message, insert_message_area,
    insert_network_area, insert_network_duplicate_log, insert_network_link, insert_network_message,
    insert_network_nodelist_entry, insert_network_packet, insert_network_path_node,
    insert_network_poll_log, insert_network_profile, insert_network_seen_by,
    insert_network_subscription, insert_oxidenet_application, insert_oxidenet_credential,
    insert_oxidenet_node, insert_session, insert_user, list_auth_attempts, list_door_definitions,
    list_door_provider_credentials, list_message_areas, list_messages, list_network_areas,
    list_network_duplicates, list_network_links, list_network_messages,
    list_network_nodelist_entries, list_network_packets, list_network_path, list_network_poll_logs,
    list_network_profiles, list_network_seen_by, list_network_subscriptions,
    list_oxidenet_applications, list_oxidenet_credentials_for_node, list_oxidenet_nodes,
    list_users, read_schema_version,
};

#[derive(Debug, Clone, Deserialize)]
struct ImportSchema {
    schema_version: i64,
    users: Vec<ImportUserRecord>,
    #[serde(default)]
    auth_attempts: Vec<ImportAuthAttemptRecord>,
    message_areas: Vec<ImportMessageAreaRecord>,
    messages: Vec<ImportMessageRecord>,
    sessions: Vec<ImportSessionRecord>,
    doors: Vec<ImportDoorDefinitionRecord>,
    door_runs: Vec<ImportDoorRunRecord>,
    #[serde(default)]
    door_provider_credentials: Vec<ImportDoorProviderCredentialRecord>,
    audit_events: Vec<ImportAuditEventRecord>,
    #[serde(default)]
    network_profiles: Vec<ImportNetworkProfileRecord>,
    #[serde(default)]
    network_links: Vec<ImportNetworkLinkRecord>,
    #[serde(default)]
    network_areas: Vec<ImportNetworkAreaRecord>,
    #[serde(default)]
    network_packets: Vec<ImportNetworkPacketRecord>,
    #[serde(default)]
    network_messages: Vec<ImportNetworkMessageRecord>,
    #[serde(default)]
    network_seen_by: Vec<ImportNetworkSeenByNode>,
    #[serde(default)]
    network_path: Vec<ImportNetworkPathNode>,
    #[serde(default)]
    network_duplicate_log: Vec<ImportNetworkDuplicateLogRecord>,
    #[serde(default)]
    network_poll_log: Vec<ImportNetworkPollLogRecord>,
    #[serde(default)]
    network_area_subscriptions: Vec<ImportNetworkSubscriptionRecord>,
    #[serde(default)]
    network_nodelist: Vec<ImportNetworkNodelistRecord>,
    #[serde(default)]
    oxidenet_applications: Vec<ImportOxideNetApplicationRecord>,
    #[serde(default)]
    oxidenet_nodes: Vec<ImportOxideNetNodeRecord>,
    #[serde(default)]
    oxidenet_credentials: Vec<ImportOxideNetCredentialRecord>,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportAuthAttemptRecord {
    scope: String,
    scope_key: String,
    failed_count: i64,
    first_failed_at: Option<String>,
    last_failed_at: Option<String>,
    locked_until: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportUserRecord {
    id: String,
    alias: String,
    real_name: String,
    email: Option<String>,
    password_hash: String,
    security_level: i64,
    is_sysop: bool,
    created_at: String,
    last_login_at: Option<String>,
    total_calls: i64,
    time_bank_minutes: i64,
    status: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportMessageAreaRecord {
    id: String,
    key: String,
    name: String,
    description: String,
    kind: String,
    network_id: Option<String>,
    read_security_level: i64,
    post_security_level: i64,
    moderated: bool,
    enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportMessageRecord {
    id: String,
    area_id: String,
    author_user_id: String,
    #[serde(default = "default_local_author_kind")]
    author_kind: String,
    #[serde(default)]
    author_display_name: String,
    #[serde(default)]
    author_network_address: Option<String>,
    to_user_id: Option<String>,
    subject: String,
    body: String,
    created_at: String,
    reply_to_id: Option<String>,
    network_message_id: Option<String>,
    visibility: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportSessionRecord {
    id: String,
    node_number: i64,
    user_id: Option<String>,
    transport: String,
    remote_address: String,
    remote_ip: Option<String>,
    remote_port: Option<i64>,
    started_at: String,
    ended_at: Option<String>,
    disconnect_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportDoorDefinitionRecord {
    id: String,
    key: String,
    name: String,
    runner: String,
    working_dir: String,
    command: String,
    drop_file: String,
    exclusive: bool,
    time_limit_minutes: i64,
    enabled: bool,
    #[serde(default)]
    min_security_level: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportDoorRunRecord {
    id: String,
    door_id: String,
    user_id: String,
    node_number: i64,
    started_at: String,
    ended_at: Option<String>,
    exit_code: Option<i64>,
    timed_out: bool,
    disconnect_forced: bool,
    bytes_in: i64,
    bytes_out: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportDoorProviderCredentialRecord {
    id: String,
    door_id: String,
    provider_name: String,
    credential_ref: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportAuditEventRecord {
    id: String,
    created_at: String,
    event_type: String,
    user_id: Option<String>,
    node_number: Option<i64>,
    details: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportNetworkProfileRecord {
    id: String,
    key: String,
    name: String,
    adapter: String,
    local_zone: i64,
    local_net: i64,
    local_node: i64,
    local_point: i64,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportNetworkLinkRecord {
    id: String,
    key: String,
    network_id: String,
    address: String,
    host: String,
    binkp_port: i64,
    password: String,
    poll_schedule_minutes: i64,
    compression: String,
    transport_security: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportNetworkAreaRecord {
    id: String,
    network_id: String,
    area_tag: String,
    local_area_id: String,
    description: String,
    read_only: bool,
    subscribed: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportNetworkPacketRecord {
    id: String,
    network_id: String,
    direction: String,
    link_id: Option<String>,
    filename: String,
    sha256: String,
    size_bytes: i64,
    status: String,
    error_message: Option<String>,
    received_at: Option<String>,
    processed_at: Option<String>,
    created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportNetworkMessageRecord {
    id: String,
    network_id: String,
    local_message_id: Option<String>,
    message_type: String,
    area_tag: Option<String>,
    origin_address: String,
    destination_address: Option<String>,
    from_name: String,
    to_name: Option<String>,
    subject: String,
    raw_text: Vec<u8>,
    display_body: String,
    msgid: Option<String>,
    replyid: Option<String>,
    created_at: String,
    imported_at: Option<String>,
    exported_at: Option<String>,
    duplicate_hash: Option<String>,
    packet_id: Option<String>,
    status: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportNetworkSeenByNode {
    id: String,
    message_id: String,
    network_id: String,
    zone: i64,
    net: i64,
    node: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportNetworkPathNode {
    id: String,
    message_id: String,
    network_id: String,
    sequence: i64,
    zone: i64,
    net: i64,
    node: i64,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportNetworkDuplicateLogRecord {
    id: String,
    network_id: String,
    duplicate_hash: String,
    msgid: Option<String>,
    area_tag: Option<String>,
    origin_address: String,
    detected_at: String,
    action: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportNetworkPollLogRecord {
    id: String,
    link_id: String,
    started_at: String,
    ended_at: Option<String>,
    direction: String,
    status: String,
    bytes_in: i64,
    bytes_out: i64,
    packets_in: i64,
    packets_out: i64,
    error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportNetworkSubscriptionRecord {
    id: String,
    area_id: String,
    link_id: String,
    subscribed: bool,
    subscribed_at: String,
    unsubscribed_at: Option<String>,
    source: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportNetworkNodelistRecord {
    id: String,
    network_id: String,
    zone: i64,
    net: i64,
    node: i64,
    point: i64,
    parsed_name: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    sysop_name: Option<String>,
    #[serde(default)]
    phone: Option<String>,
    #[serde(default)]
    speed: Option<String>,
    #[serde(default)]
    flags: String,
    raw_entry: String,
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportOxideNetApplicationRecord {
    id: String,
    created_at: String,
    updated_at: String,
    submitted_at: Option<String>,
    reviewed_at: Option<String>,
    status: String,
    applicant_user_id: Option<String>,
    board_name: String,
    sysop_alias: String,
    contact_email: String,
    host: String,
    binkp_port: i64,
    telnet_host: Option<String>,
    telnet_port: Option<i64>,
    software: String,
    software_version: String,
    timezone: String,
    region: String,
    description: String,
    reason: String,
    policy_version: String,
    policy_accepted_at: Option<String>,
    admin_notes: String,
    reviewed_by_user_id: Option<String>,
    assigned_address: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportOxideNetNodeRecord {
    id: String,
    application_id: Option<String>,
    network_key: String,
    address: String,
    zone: i64,
    net: i64,
    node: i64,
    point: i64,
    hub_address: String,
    board_name: String,
    sysop_alias: String,
    contact_email: String,
    host: String,
    binkp_port: i64,
    telnet_host: Option<String>,
    telnet_port: Option<i64>,
    software: String,
    software_version: String,
    status: String,
    created_at: String,
    updated_at: String,
    activated_at: Option<String>,
    suspended_at: Option<String>,
    retired_at: Option<String>,
    last_poll_at: Option<String>,
    last_successful_poll_at: Option<String>,
    flags: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ImportOxideNetCredentialRecord {
    id: String,
    node_id: String,
    credential_kind: String,
    secret_hash: String,
    created_at: String,
    rotated_at: Option<String>,
    expires_at: Option<String>,
    status: String,
}

impl From<ImportUserRecord> for UserRecord {
    fn from(record: ImportUserRecord) -> Self {
        Self {
            id: record.id,
            alias: record.alias,
            real_name: record.real_name,
            email: record.email,
            password_hash: record.password_hash,
            security_level: record.security_level,
            is_sysop: record.is_sysop,
            created_at: record.created_at,
            last_login_at: record.last_login_at,
            total_calls: record.total_calls,
            time_bank_minutes: record.time_bank_minutes,
            status: record.status,
        }
    }
}

impl From<ImportAuthAttemptRecord> for AuthAttemptRecord {
    fn from(record: ImportAuthAttemptRecord) -> Self {
        Self {
            scope: record.scope,
            scope_key: record.scope_key,
            failed_count: record.failed_count,
            first_failed_at: record.first_failed_at,
            last_failed_at: record.last_failed_at,
            locked_until: record.locked_until,
        }
    }
}

impl From<ImportMessageAreaRecord> for MessageAreaRecord {
    fn from(record: ImportMessageAreaRecord) -> Self {
        Self {
            id: record.id,
            key: record.key,
            name: record.name,
            description: record.description,
            kind: record.kind,
            network_id: record.network_id,
            read_security_level: record.read_security_level,
            post_security_level: record.post_security_level,
            moderated: record.moderated,
            enabled: record.enabled,
        }
    }
}

impl From<ImportMessageRecord> for MessageRecord {
    fn from(record: ImportMessageRecord) -> Self {
        Self {
            id: record.id,
            area_id: record.area_id,
            author_user_id: record.author_user_id,
            author_kind: record.author_kind,
            author_display_name: record.author_display_name,
            author_network_address: record.author_network_address,
            to_user_id: record.to_user_id,
            subject: record.subject,
            body: record.body,
            created_at: record.created_at,
            reply_to_id: record.reply_to_id,
            network_message_id: record.network_message_id,
            visibility: record.visibility,
        }
    }
}

fn default_local_author_kind() -> String {
    "local".to_string()
}

impl From<ImportSessionRecord> for SessionRecord {
    fn from(record: ImportSessionRecord) -> Self {
        Self {
            id: record.id,
            node_number: record.node_number,
            user_id: record.user_id,
            transport: record.transport,
            remote_address: record.remote_address,
            remote_ip: record.remote_ip,
            remote_port: record.remote_port,
            started_at: record.started_at,
            ended_at: record.ended_at,
            disconnect_reason: record.disconnect_reason,
        }
    }
}

impl From<ImportDoorDefinitionRecord> for DoorDefinitionRecord {
    fn from(record: ImportDoorDefinitionRecord) -> Self {
        Self {
            id: record.id,
            key: record.key,
            name: record.name,
            runner: record.runner,
            working_dir: record.working_dir,
            command: record.command,
            drop_file: record.drop_file,
            exclusive: record.exclusive,
            time_limit_minutes: record.time_limit_minutes,
            enabled: record.enabled,
            min_security_level: record.min_security_level,
        }
    }
}

impl From<ImportDoorRunRecord> for DoorRunRecord {
    fn from(record: ImportDoorRunRecord) -> Self {
        Self {
            id: record.id,
            door_id: record.door_id,
            user_id: record.user_id,
            node_number: record.node_number,
            started_at: record.started_at,
            ended_at: record.ended_at,
            exit_code: record.exit_code,
            timed_out: record.timed_out,
            disconnect_forced: record.disconnect_forced,
            bytes_in: record.bytes_in,
            bytes_out: record.bytes_out,
        }
    }
}

impl From<ImportDoorProviderCredentialRecord> for DoorProviderCredentialRecord {
    fn from(record: ImportDoorProviderCredentialRecord) -> Self {
        Self {
            id: record.id,
            door_id: record.door_id,
            provider_name: record.provider_name,
            credential_ref: record.credential_ref,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

impl From<ImportAuditEventRecord> for AuditEventRecord {
    fn from(record: ImportAuditEventRecord) -> Self {
        Self {
            id: record.id,
            created_at: record.created_at,
            event_type: record.event_type,
            user_id: record.user_id,
            node_number: record.node_number,
            details: record.details,
        }
    }
}

impl From<ImportNetworkProfileRecord> for NetworkProfileRecord {
    fn from(record: ImportNetworkProfileRecord) -> Self {
        Self {
            id: record.id,
            key: record.key,
            name: record.name,
            adapter: record.adapter,
            local_zone: record.local_zone,
            local_net: record.local_net,
            local_node: record.local_node,
            local_point: record.local_point,
            enabled: record.enabled,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

impl From<ImportNetworkLinkRecord> for NetworkLinkRecord {
    fn from(record: ImportNetworkLinkRecord) -> Self {
        Self {
            id: record.id,
            key: record.key,
            network_id: record.network_id,
            address: record.address,
            host: record.host,
            binkp_port: record.binkp_port,
            password: record.password,
            poll_schedule_minutes: record.poll_schedule_minutes,
            compression: record.compression,
            transport_security: record.transport_security,
            enabled: record.enabled,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

impl From<ImportNetworkAreaRecord> for NetworkAreaRecord {
    fn from(record: ImportNetworkAreaRecord) -> Self {
        Self {
            id: record.id,
            network_id: record.network_id,
            area_tag: record.area_tag,
            local_area_id: record.local_area_id,
            description: record.description,
            read_only: record.read_only,
            subscribed: record.subscribed,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

impl From<ImportNetworkPacketRecord> for NetworkPacketRecord {
    fn from(record: ImportNetworkPacketRecord) -> Self {
        Self {
            id: record.id,
            network_id: record.network_id,
            direction: record.direction,
            link_id: record.link_id,
            filename: record.filename,
            sha256: record.sha256,
            size_bytes: record.size_bytes,
            status: record.status,
            error_message: record.error_message,
            received_at: record.received_at,
            processed_at: record.processed_at,
            created_at: record.created_at,
        }
    }
}

impl From<ImportNetworkMessageRecord> for NetworkMessageRecord {
    fn from(record: ImportNetworkMessageRecord) -> Self {
        Self {
            id: record.id,
            network_id: record.network_id,
            local_message_id: record.local_message_id,
            message_type: record.message_type,
            area_tag: record.area_tag,
            origin_address: record.origin_address,
            destination_address: record.destination_address,
            from_name: record.from_name,
            to_name: record.to_name,
            subject: record.subject,
            raw_text: record.raw_text,
            display_body: record.display_body,
            msgid: record.msgid,
            replyid: record.replyid,
            created_at: record.created_at,
            imported_at: record.imported_at,
            exported_at: record.exported_at,
            duplicate_hash: record.duplicate_hash,
            packet_id: record.packet_id,
            status: record.status,
        }
    }
}

impl From<ImportNetworkSeenByNode> for NetworkSeenByNode {
    fn from(record: ImportNetworkSeenByNode) -> Self {
        Self {
            id: record.id,
            message_id: record.message_id,
            network_id: record.network_id,
            zone: record.zone,
            net: record.net,
            node: record.node,
        }
    }
}

impl From<ImportNetworkPathNode> for NetworkPathNode {
    fn from(record: ImportNetworkPathNode) -> Self {
        Self {
            id: record.id,
            message_id: record.message_id,
            network_id: record.network_id,
            sequence: record.sequence,
            zone: record.zone,
            net: record.net,
            node: record.node,
        }
    }
}

impl From<ImportNetworkDuplicateLogRecord> for NetworkDuplicateLogRecord {
    fn from(record: ImportNetworkDuplicateLogRecord) -> Self {
        Self {
            id: record.id,
            network_id: record.network_id,
            duplicate_hash: record.duplicate_hash,
            msgid: record.msgid,
            area_tag: record.area_tag,
            origin_address: record.origin_address,
            detected_at: record.detected_at,
            action: record.action,
        }
    }
}

impl From<ImportNetworkPollLogRecord> for NetworkPollLogRecord {
    fn from(record: ImportNetworkPollLogRecord) -> Self {
        Self {
            id: record.id,
            link_id: record.link_id,
            started_at: record.started_at,
            ended_at: record.ended_at,
            direction: record.direction,
            status: record.status,
            bytes_in: record.bytes_in,
            bytes_out: record.bytes_out,
            packets_in: record.packets_in,
            packets_out: record.packets_out,
            error_message: record.error_message,
        }
    }
}

impl From<ImportNetworkSubscriptionRecord> for NetworkSubscriptionRecord {
    fn from(record: ImportNetworkSubscriptionRecord) -> Self {
        Self {
            id: record.id,
            area_id: record.area_id,
            link_id: record.link_id,
            subscribed: record.subscribed,
            subscribed_at: record.subscribed_at,
            unsubscribed_at: record.unsubscribed_at,
            source: record.source,
        }
    }
}

impl From<ImportNetworkNodelistRecord> for NetworkNodelistRecord {
    fn from(record: ImportNetworkNodelistRecord) -> Self {
        Self {
            id: record.id,
            network_id: record.network_id,
            zone: record.zone,
            net: record.net,
            node: record.node,
            point: record.point,
            parsed_name: record.parsed_name,
            location: record.location,
            sysop_name: record.sysop_name,
            phone: record.phone,
            speed: record.speed,
            flags: record.flags,
            raw_entry: record.raw_entry,
            updated_at: record.updated_at,
        }
    }
}

impl From<ImportOxideNetApplicationRecord> for OxideNetApplicationRecord {
    fn from(record: ImportOxideNetApplicationRecord) -> Self {
        Self {
            id: record.id,
            created_at: record.created_at,
            updated_at: record.updated_at,
            submitted_at: record.submitted_at,
            reviewed_at: record.reviewed_at,
            status: record.status,
            applicant_user_id: record.applicant_user_id,
            board_name: record.board_name,
            sysop_alias: record.sysop_alias,
            contact_email: record.contact_email,
            host: record.host,
            binkp_port: record.binkp_port,
            telnet_host: record.telnet_host,
            telnet_port: record.telnet_port,
            software: record.software,
            software_version: record.software_version,
            timezone: record.timezone,
            region: record.region,
            description: record.description,
            reason: record.reason,
            policy_version: record.policy_version,
            policy_accepted_at: record.policy_accepted_at,
            admin_notes: record.admin_notes,
            reviewed_by_user_id: record.reviewed_by_user_id,
            assigned_address: record.assigned_address,
        }
    }
}

impl From<ImportOxideNetNodeRecord> for OxideNetNodeRecord {
    fn from(record: ImportOxideNetNodeRecord) -> Self {
        Self {
            id: record.id,
            application_id: record.application_id,
            network_key: record.network_key,
            address: record.address,
            zone: record.zone,
            net: record.net,
            node: record.node,
            point: record.point,
            hub_address: record.hub_address,
            board_name: record.board_name,
            sysop_alias: record.sysop_alias,
            contact_email: record.contact_email,
            host: record.host,
            binkp_port: record.binkp_port,
            telnet_host: record.telnet_host,
            telnet_port: record.telnet_port,
            software: record.software,
            software_version: record.software_version,
            status: record.status,
            created_at: record.created_at,
            updated_at: record.updated_at,
            activated_at: record.activated_at,
            suspended_at: record.suspended_at,
            retired_at: record.retired_at,
            last_poll_at: record.last_poll_at,
            last_successful_poll_at: record.last_successful_poll_at,
            flags: record.flags,
        }
    }
}

impl From<ImportOxideNetCredentialRecord> for OxideNetCredentialRecord {
    fn from(record: ImportOxideNetCredentialRecord) -> Self {
        Self {
            id: record.id,
            node_id: record.node_id,
            credential_kind: record.credential_kind,
            secret_hash: record.secret_hash,
            created_at: record.created_at,
            rotated_at: record.rotated_at,
            expires_at: record.expires_at,
            status: record.status,
        }
    }
}

fn db_scalar_i64(db: &Db, sql: &str) -> CliResult<i64> {
    let result = db.execute(sql)?;
    let value = result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .ok_or_else(|| CliError::Message(format!("query returned no scalar value: {sql}")))?;
    match value {
        oxidebbs_db::Value::Int64(value) => Ok(*value),
        other => Err(CliError::Message(format!(
            "query returned non-int scalar for {sql}: {other:?}"
        ))),
    }
}

fn text_value(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

fn opt_text_value(value: &Value) -> Option<String> {
    match value {
        Value::Text(text) => Some(text.clone()),
        _ => None,
    }
}

fn int_value(value: &Value) -> i64 {
    match value {
        Value::Int64(value) => *value,
        _ => 0,
    }
}

fn opt_int_value(value: &Value) -> Option<i64> {
    match value {
        Value::Int64(value) => Some(*value),
        _ => None,
    }
}

fn bool_value(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        _ => false,
    }
}

fn list_all_sessions_for_export(db: &Db) -> CliResult<Vec<SessionRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), node_number, UUID_TO_STRING(user_id), transport, remote_address, CAST(remote_ip AS TEXT), remote_port, CAST(started_at AS TEXT), CAST(ended_at AS TEXT), disconnect_reason
         FROM sessions ORDER BY started_at DESC",
    )?;
    Ok(result
        .rows()
        .iter()
        .map(|row| {
            let values = row.values();
            SessionRecord {
                id: text_value(&values[0]),
                node_number: int_value(&values[1]),
                user_id: opt_text_value(&values[2]),
                transport: text_value(&values[3]),
                remote_address: text_value(&values[4]),
                remote_ip: opt_text_value(&values[5]),
                remote_port: opt_int_value(&values[6]),
                started_at: text_value(&values[7]),
                ended_at: opt_text_value(&values[8]),
                disconnect_reason: opt_text_value(&values[9]),
            }
        })
        .collect())
}

fn list_all_door_runs_for_export(db: &Db) -> CliResult<Vec<DoorRunRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(door_id), UUID_TO_STRING(user_id), node_number, CAST(started_at AS TEXT), CAST(ended_at AS TEXT), exit_code, timed_out, disconnect_forced, bytes_in, bytes_out
         FROM door_runs ORDER BY started_at DESC",
    )?;
    Ok(result
        .rows()
        .iter()
        .map(|row| {
            let values = row.values();
            DoorRunRecord {
                id: text_value(&values[0]),
                door_id: text_value(&values[1]),
                user_id: text_value(&values[2]),
                node_number: int_value(&values[3]),
                started_at: text_value(&values[4]),
                ended_at: opt_text_value(&values[5]),
                exit_code: opt_int_value(&values[6]),
                timed_out: bool_value(&values[7]),
                disconnect_forced: bool_value(&values[8]),
                bytes_in: int_value(&values[9]),
                bytes_out: int_value(&values[10]),
            }
        })
        .collect())
}

fn list_all_audit_events_for_export(db: &Db) -> CliResult<Vec<AuditEventRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), CAST(created_at AS TEXT), event_type, UUID_TO_STRING(user_id), node_number, details
         FROM audit_events ORDER BY created_at DESC",
    )?;
    Ok(result
        .rows()
        .iter()
        .map(|row| {
            let values = row.values();
            AuditEventRecord {
                id: text_value(&values[0]),
                created_at: text_value(&values[1]),
                event_type: text_value(&values[2]),
                user_id: opt_text_value(&values[3]),
                node_number: opt_int_value(&values[4]),
                details: text_value(&values[5]),
            }
        })
        .collect())
}

fn session_json(session: &oxidebbs_db::SessionRecord) -> JsonValue {
    serde_json::json!({
        "id": session.id,
        "node_number": session.node_number,
        "user_id": session.user_id,
        "transport": session.transport,
        "remote_address": session.remote_address,
        "remote_ip": session.remote_ip,
        "remote_port": session.remote_port,
        "started_at": session.started_at,
        "ended_at": session.ended_at,
        "disconnect_reason": session.disconnect_reason
    })
}

fn message_json(message: &oxidebbs_db::MessageRecord) -> JsonValue {
    serde_json::json!({
        "id": message.id,
        "area_id": message.area_id,
        "author_user_id": message.author_user_id,
        "author_kind": message.author_kind,
        "author_display_name": message.author_display_name,
        "author_network_address": message.author_network_address,
        "to_user_id": message.to_user_id,
        "subject": message.subject,
        "body": message.body,
        "created_at": message.created_at,
        "reply_to_id": message.reply_to_id,
        "network_message_id": message.network_message_id,
        "visibility": message.visibility
    })
}

fn area_json(area: &oxidebbs_db::MessageAreaRecord) -> JsonValue {
    serde_json::json!({
        "id": area.id,
        "key": area.key,
        "name": area.name,
        "description": area.description,
        "kind": area.kind,
        "network_id": area.network_id,
        "read_security_level": area.read_security_level,
        "post_security_level": area.post_security_level,
        "moderated": area.moderated,
        "enabled": area.enabled
    })
}

fn user_json(user: &oxidebbs_db::UserRecord) -> JsonValue {
    serde_json::json!({
        "id": user.id,
        "alias": user.alias,
        "real_name": user.real_name,
        "email": user.email,
        "password_hash": user.password_hash,
        "security_level": user.security_level,
        "is_sysop": user.is_sysop,
        "created_at": user.created_at,
        "last_login_at": user.last_login_at,
        "total_calls": user.total_calls,
        "time_bank_minutes": user.time_bank_minutes,
        "status": user.status
    })
}

fn door_json(door: &oxidebbs_db::DoorDefinitionRecord) -> JsonValue {
    serde_json::json!({
        "id": door.id,
        "key": door.key,
        "name": door.name,
        "runner": door.runner,
        "working_dir": door.working_dir,
        "command": door.command,
        "drop_file": door.drop_file,
        "exclusive": door.exclusive,
        "time_limit_minutes": door.time_limit_minutes,
        "enabled": door.enabled,
        "min_security_level": door.min_security_level
    })
}

fn door_run_json(run: &oxidebbs_db::DoorRunRecord) -> JsonValue {
    serde_json::json!({
        "id": run.id,
        "door_id": run.door_id,
        "user_id": run.user_id,
        "node_number": run.node_number,
        "started_at": run.started_at,
        "ended_at": run.ended_at,
        "exit_code": run.exit_code,
        "timed_out": run.timed_out,
        "disconnect_forced": run.disconnect_forced,
        "bytes_in": run.bytes_in,
        "bytes_out": run.bytes_out
    })
}

fn door_provider_credential_json(credential: &DoorProviderCredentialRecord) -> JsonValue {
    serde_json::json!({
        "id": credential.id,
        "door_id": credential.door_id,
        "provider_name": credential.provider_name,
        "credential_ref": "[redacted]",
        "created_at": credential.created_at,
        "updated_at": credential.updated_at
    })
}

fn db_stats(db: &Db, active_sessions: i64) -> CliResult<JsonValue> {
    let open_sessions = db_scalar_i64(db, "SELECT COUNT(*) FROM sessions WHERE ended_at IS NULL")?;
    Ok(serde_json::json!({
        "schema_version": read_schema_version(db)?,
        "users": db_scalar_i64(db, "SELECT COUNT(*) FROM users")?,
        "message_areas": db_scalar_i64(db, "SELECT COUNT(*) FROM message_areas")?,
        "messages": db_scalar_i64(db, "SELECT COUNT(*) FROM messages")?,
        "sessions": db_scalar_i64(db, "SELECT COUNT(*) FROM sessions")?,
        "active_sessions": active_sessions,
        "open_sessions": open_sessions,
        "auth_attempts": db_scalar_i64(db, "SELECT COUNT(*) FROM auth_attempts")?,
        "doors": db_scalar_i64(db, "SELECT COUNT(*) FROM doors")?,
        "door_runs": db_scalar_i64(db, "SELECT COUNT(*) FROM door_runs")?,
        "audit_events": db_scalar_i64(db, "SELECT COUNT(*) FROM audit_events")?
    }))
}

fn live_active_session_count(ctx: &AppContext) -> CliResult<i64> {
    match request_status(&ctx.config.paths.runtime) {
        Ok(ControlResponse::Status { status, .. }) => Ok(i64::try_from(status.active_nodes)
            .map_err(|error| {
                CliError::Message(format!("active node count is out of range: {error}"))
            })?),
        Ok(ControlResponse::Error { error, .. }) => Err(CliError::Message(format!(
            "control socket reported error: {error}"
        ))),
        Ok(ControlResponse::Ok { .. }) => Err(CliError::Message(
            "control socket returned unexpected response to status request".to_string(),
        )),
        Ok(ControlResponse::Nodes { .. }) => Err(CliError::Message(
            "control socket returned unexpected response to status request".to_string(),
        )),
        Err(error) if error.is_unreachable() => Ok(0),
        Err(error) => Err(CliError::Message(format!("status request failed: {error}"))),
    }
}

fn print_stats(stats: &JsonValue) {
    if let Some(object) = stats.as_object() {
        for (key, value) in object {
            println!("{key}: {value}");
        }
    }
}

fn auth_attempt_json(attempt: &AuthAttemptRecord) -> JsonValue {
    serde_json::json!({
        "scope": attempt.scope,
        "scope_key": attempt.scope_key,
        "failed_count": attempt.failed_count,
        "first_failed_at": attempt.first_failed_at,
        "last_failed_at": attempt.last_failed_at,
        "locked_until": attempt.locked_until
    })
}

fn audit_json(event: &oxidebbs_db::AuditEventRecord) -> JsonValue {
    serde_json::json!({
        "id": event.id,
        "created_at": event.created_at,
        "event_type": event.event_type,
        "user_id": event.user_id,
        "node_number": event.node_number,
        "details": event.details
    })
}

fn network_profile_json(profile: &NetworkProfileRecord) -> JsonValue {
    serde_json::json!({
        "id": profile.id,
        "key": profile.key,
        "name": profile.name,
        "adapter": profile.adapter,
        "local_zone": profile.local_zone,
        "local_net": profile.local_net,
        "local_node": profile.local_node,
        "local_point": profile.local_point,
        "enabled": profile.enabled,
        "created_at": profile.created_at,
        "updated_at": profile.updated_at
    })
}

fn network_link_json(link: &NetworkLinkRecord) -> JsonValue {
    serde_json::json!({
        "id": link.id,
        "key": link.key,
        "network_id": link.network_id,
        "address": link.address,
        "host": link.host,
        "binkp_port": link.binkp_port,
        "password": link.password,
        "poll_schedule_minutes": link.poll_schedule_minutes,
        "compression": link.compression,
        "transport_security": link.transport_security,
        "enabled": link.enabled,
        "created_at": link.created_at,
        "updated_at": link.updated_at
    })
}

fn network_area_json(area: &NetworkAreaRecord) -> JsonValue {
    serde_json::json!({
        "id": area.id,
        "network_id": area.network_id,
        "area_tag": area.area_tag,
        "local_area_id": area.local_area_id,
        "description": area.description,
        "read_only": area.read_only,
        "subscribed": area.subscribed,
        "created_at": area.created_at,
        "updated_at": area.updated_at
    })
}

fn network_packet_json(packet: &NetworkPacketRecord) -> JsonValue {
    serde_json::json!({
        "id": packet.id,
        "network_id": packet.network_id,
        "direction": packet.direction,
        "link_id": packet.link_id,
        "filename": packet.filename,
        "sha256": packet.sha256,
        "size_bytes": packet.size_bytes,
        "status": packet.status,
        "error_message": packet.error_message,
        "received_at": packet.received_at,
        "processed_at": packet.processed_at,
        "created_at": packet.created_at
    })
}

fn network_message_json(message: &NetworkMessageRecord) -> JsonValue {
    serde_json::json!({
        "id": message.id,
        "network_id": message.network_id,
        "local_message_id": message.local_message_id,
        "message_type": message.message_type,
        "area_tag": message.area_tag,
        "origin_address": message.origin_address,
        "destination_address": message.destination_address,
        "from_name": message.from_name,
        "to_name": message.to_name,
        "subject": message.subject,
        "raw_text": message.raw_text,
        "display_body": message.display_body,
        "msgid": message.msgid,
        "replyid": message.replyid,
        "created_at": message.created_at,
        "imported_at": message.imported_at,
        "exported_at": message.exported_at,
        "duplicate_hash": message.duplicate_hash,
        "packet_id": message.packet_id,
        "status": message.status
    })
}

fn network_seen_by_json(node: &NetworkSeenByNode) -> JsonValue {
    serde_json::json!({
        "id": node.id,
        "message_id": node.message_id,
        "network_id": node.network_id,
        "zone": node.zone,
        "net": node.net,
        "node": node.node
    })
}

fn network_path_json(node: &NetworkPathNode) -> JsonValue {
    serde_json::json!({
        "id": node.id,
        "message_id": node.message_id,
        "network_id": node.network_id,
        "sequence": node.sequence,
        "zone": node.zone,
        "net": node.net,
        "node": node.node
    })
}

fn network_duplicate_json(log: &NetworkDuplicateLogRecord) -> JsonValue {
    serde_json::json!({
        "id": log.id,
        "network_id": log.network_id,
        "duplicate_hash": log.duplicate_hash,
        "msgid": log.msgid,
        "area_tag": log.area_tag,
        "origin_address": log.origin_address,
        "detected_at": log.detected_at,
        "action": log.action
    })
}

fn network_poll_json(log: &NetworkPollLogRecord) -> JsonValue {
    serde_json::json!({
        "id": log.id,
        "link_id": log.link_id,
        "started_at": log.started_at,
        "ended_at": log.ended_at,
        "direction": log.direction,
        "status": log.status,
        "bytes_in": log.bytes_in,
        "bytes_out": log.bytes_out,
        "packets_in": log.packets_in,
        "packets_out": log.packets_out,
        "error_message": log.error_message
    })
}

fn network_subscription_json(subscription: &NetworkSubscriptionRecord) -> JsonValue {
    serde_json::json!({
        "id": subscription.id,
        "area_id": subscription.area_id,
        "link_id": subscription.link_id,
        "subscribed": subscription.subscribed,
        "subscribed_at": subscription.subscribed_at,
        "unsubscribed_at": subscription.unsubscribed_at,
        "source": subscription.source
    })
}

fn network_nodelist_json(entry: &NetworkNodelistRecord) -> JsonValue {
    serde_json::json!({
        "id": entry.id,
        "network_id": entry.network_id,
        "zone": entry.zone,
        "net": entry.net,
        "node": entry.node,
        "point": entry.point,
        "parsed_name": entry.parsed_name,
        "location": entry.location,
        "sysop_name": entry.sysop_name,
        "phone": entry.phone,
        "speed": entry.speed,
        "flags": entry.flags,
        "raw_entry": entry.raw_entry,
        "updated_at": entry.updated_at
    })
}

fn oxidenet_application_json(application: &OxideNetApplicationRecord) -> JsonValue {
    serde_json::json!({
        "id": application.id,
        "created_at": application.created_at,
        "updated_at": application.updated_at,
        "submitted_at": application.submitted_at,
        "reviewed_at": application.reviewed_at,
        "status": application.status,
        "applicant_user_id": application.applicant_user_id,
        "board_name": application.board_name,
        "sysop_alias": application.sysop_alias,
        "contact_email": application.contact_email,
        "host": application.host,
        "binkp_port": application.binkp_port,
        "telnet_host": application.telnet_host,
        "telnet_port": application.telnet_port,
        "software": application.software,
        "software_version": application.software_version,
        "timezone": application.timezone,
        "region": application.region,
        "description": application.description,
        "reason": application.reason,
        "policy_version": application.policy_version,
        "policy_accepted_at": application.policy_accepted_at,
        "admin_notes": application.admin_notes,
        "reviewed_by_user_id": application.reviewed_by_user_id,
        "assigned_address": application.assigned_address
    })
}

fn oxidenet_node_json(node: &OxideNetNodeRecord) -> JsonValue {
    serde_json::json!({
        "id": node.id,
        "application_id": node.application_id,
        "network_key": node.network_key,
        "address": node.address,
        "zone": node.zone,
        "net": node.net,
        "node": node.node,
        "point": node.point,
        "hub_address": node.hub_address,
        "board_name": node.board_name,
        "sysop_alias": node.sysop_alias,
        "contact_email": node.contact_email,
        "host": node.host,
        "binkp_port": node.binkp_port,
        "telnet_host": node.telnet_host,
        "telnet_port": node.telnet_port,
        "software": node.software,
        "software_version": node.software_version,
        "status": node.status,
        "created_at": node.created_at,
        "updated_at": node.updated_at,
        "activated_at": node.activated_at,
        "suspended_at": node.suspended_at,
        "retired_at": node.retired_at,
        "last_poll_at": node.last_poll_at,
        "last_successful_poll_at": node.last_successful_poll_at,
        "flags": node.flags
    })
}

fn oxidenet_credential_json(credential: &OxideNetCredentialRecord) -> JsonValue {
    serde_json::json!({
        "id": credential.id,
        "node_id": credential.node_id,
        "credential_kind": credential.credential_kind,
        "secret_hash": credential.secret_hash,
        "created_at": credential.created_at,
        "rotated_at": credential.rotated_at,
        "expires_at": credential.expires_at,
        "status": credential.status
    })
}

fn db_export(db: &Db) -> CliResult<JsonValue> {
    const EXPORT_ROW_LIMIT: i64 = 1_000_000;

    let oxidenet_nodes = list_oxidenet_nodes(db, EXPORT_ROW_LIMIT)?;
    let mut oxidenet_credentials = Vec::new();
    for node in &oxidenet_nodes {
        oxidenet_credentials.extend(list_oxidenet_credentials_for_node(db, &node.id)?);
    }

    let doors = list_door_definitions(db)?;
    let mut door_provider_credentials = Vec::new();
    for door in &doors {
        door_provider_credentials.extend(list_door_provider_credentials(db, &door.id)?);
    }

    Ok(serde_json::json!({
        "schema_version": read_schema_version(db)?,
        "users": list_users(db)?.iter().map(user_json).collect::<Vec<_>>(),
        "auth_attempts": list_auth_attempts(db)?.iter().map(auth_attempt_json).collect::<Vec<_>>(),
        "message_areas": list_message_areas(db)?.iter().map(area_json).collect::<Vec<_>>(),
        "messages": list_messages(db)?.iter().map(message_json).collect::<Vec<_>>(),
        "sessions": list_all_sessions_for_export(db)?.iter().map(session_json).collect::<Vec<_>>(),
        "doors": doors.iter().map(door_json).collect::<Vec<_>>(),
        "door_runs": list_all_door_runs_for_export(db)?.iter().map(door_run_json).collect::<Vec<_>>(),
        "door_provider_credentials": door_provider_credentials.iter().map(door_provider_credential_json).collect::<Vec<_>>(),
        "audit_events": list_all_audit_events_for_export(db)?.iter().map(audit_json).collect::<Vec<_>>(),
        "network_profiles": list_network_profiles(db)?.iter().map(network_profile_json).collect::<Vec<_>>(),
        "network_links": list_network_links(db)?.iter().map(network_link_json).collect::<Vec<_>>(),
        "network_areas": list_network_areas(db)?.iter().map(network_area_json).collect::<Vec<_>>(),
        "network_packets": list_network_packets(db)?.iter().map(network_packet_json).collect::<Vec<_>>(),
        "network_messages": list_network_messages(db)?.iter().map(network_message_json).collect::<Vec<_>>(),
        "network_seen_by": list_network_seen_by(db)?.iter().map(network_seen_by_json).collect::<Vec<_>>(),
        "network_path": list_network_path(db)?.iter().map(network_path_json).collect::<Vec<_>>(),
        "network_duplicate_log": list_network_duplicates(db)?.iter().map(network_duplicate_json).collect::<Vec<_>>(),
        "network_poll_log": list_network_poll_logs(db)?.iter().map(network_poll_json).collect::<Vec<_>>(),
        "network_area_subscriptions": list_network_subscriptions(db)?.iter().map(network_subscription_json).collect::<Vec<_>>(),
        "network_nodelist": list_network_nodelist_entries(db)?.iter().map(network_nodelist_json).collect::<Vec<_>>(),
        "oxidenet_applications": list_oxidenet_applications(db, EXPORT_ROW_LIMIT)?.iter().map(oxidenet_application_json).collect::<Vec<_>>(),
        "oxidenet_nodes": oxidenet_nodes.iter().map(oxidenet_node_json).collect::<Vec<_>>(),
        "oxidenet_credentials": oxidenet_credentials.iter().map(oxidenet_credential_json).collect::<Vec<_>>()
    }))
}

fn parse_db_import(path: &Path, format: &str) -> CliResult<ImportSchema> {
    require_json_format(format)?;
    let body = fs::read_to_string(path)?;
    let payload = serde_json::from_str::<ImportSchema>(&body)?;
    Ok(payload)
}

fn ensure_import_target_is_schema_only(db: &Db) -> CliResult<()> {
    let system_rows = db_scalar_i64(
        db,
        "SELECT COUNT(*) FROM system_config WHERE key <> 'schema_version'",
    )?;
    if system_rows > 0 {
        return Err(CliError::Message(
            "import target contains unsupported system_config rows".to_string(),
        ));
    }

    for table in [
        "users",
        "auth_attempts",
        "message_areas",
        "messages",
        "sessions",
        "doors",
        "door_runs",
        "audit_events",
        "network_profiles",
        "network_links",
        "network_areas",
        "network_packets",
        "network_messages",
        "network_seen_by",
        "network_path",
        "network_duplicate_log",
        "network_poll_log",
        "network_area_subscriptions",
        "network_nodelist",
        "network_applications",
        "network_nodes",
        "network_credentials",
    ] {
        let count = db_scalar_i64(db, &format!("SELECT COUNT(*) FROM {table}"))?;
        if count > 0 {
            return Err(CliError::Message(format!(
                "import target must be schema-only; existing rows found in {table}"
            )));
        }
    }

    Ok(())
}

fn validate_import_payload(payload: &ImportSchema, current_schema_version: i64) -> CliResult<()> {
    if payload.schema_version != current_schema_version {
        return Err(CliError::Message(format!(
            "unsupported import schema version {}; expected {}",
            payload.schema_version, current_schema_version
        )));
    }

    let mut user_ids = HashSet::with_capacity(payload.users.len());
    for user in &payload.users {
        if !user_ids.insert(user.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate user id {} in import payload",
                user.id
            )));
        }
    }

    let mut area_ids = HashSet::with_capacity(payload.message_areas.len());
    for area in &payload.message_areas {
        if !area_ids.insert(area.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate message area id {} in import payload",
                area.id
            )));
        }
        if area.kind != "local" && area.kind != "echomail" && area.kind != "netmail" {
            return Err(CliError::Message(format!(
                "invalid message area kind {} for {}",
                area.kind, area.key
            )));
        }
    }

    let mut message_ids = HashSet::with_capacity(payload.messages.len());
    for message in &payload.messages {
        if !message_ids.insert(message.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate message id {} in import payload",
                message.id
            )));
        }
        match message.author_kind.as_str() {
            "local" | "network" | "system" => {}
            other => {
                return Err(CliError::Message(format!(
                    "message {} has invalid author_kind {}",
                    message.id, other
                )));
            }
        }
    }
    for message in &payload.messages {
        if !area_ids.contains(message.area_id.as_str()) {
            return Err(CliError::Message(format!(
                "message {} references missing area {}",
                message.id, message.area_id
            )));
        }
        if message.author_kind == "local" && !user_ids.contains(message.author_user_id.as_str()) {
            return Err(CliError::Message(format!(
                "message {} references missing author {}",
                message.id, message.author_user_id
            )));
        }
        if let Some(to_user_id) = message.to_user_id.as_deref()
            && !user_ids.contains(to_user_id)
        {
            return Err(CliError::Message(format!(
                "message {} references missing recipient {}",
                message.id, to_user_id
            )));
        }
        if let Some(reply_to_id) = message.reply_to_id.as_deref()
            && !message_ids.contains(reply_to_id)
        {
            return Err(CliError::Message(format!(
                "message {} references missing parent message {}",
                message.id, reply_to_id
            )));
        }
    }

    let mut network_profile_ids = HashSet::with_capacity(payload.network_profiles.len());
    for profile in &payload.network_profiles {
        if !network_profile_ids.insert(profile.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate network profile id {} in import payload",
                profile.id
            )));
        }
        if profile.adapter != "legacy-ftn" && profile.adapter != "oxidenet" {
            return Err(CliError::Message(format!(
                "network profile {} has invalid adapter {}",
                profile.key, profile.adapter
            )));
        }
        if profile.local_zone <= 0 || profile.local_net <= 0 || profile.local_node <= 0 {
            return Err(CliError::Message(format!(
                "network profile {} has invalid local address",
                profile.key
            )));
        }
    }

    let mut network_link_ids = HashSet::with_capacity(payload.network_links.len());
    for link in &payload.network_links {
        if !network_link_ids.insert(link.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate network link id {} in import payload",
                link.id
            )));
        }
        if !network_profile_ids.contains(link.network_id.as_str()) {
            return Err(CliError::Message(format!(
                "network link {} references missing profile {}",
                link.key, link.network_id
            )));
        }
        if link.binkp_port <= 0 || link.binkp_port > 65_535 {
            return Err(CliError::Message(format!(
                "network link {} has invalid BinkP port {}",
                link.key, link.binkp_port
            )));
        }
        match link.compression.as_str() {
            "none" | "zip" | "arj" => {}
            other => {
                return Err(CliError::Message(format!(
                    "network link {} has invalid compression {}",
                    link.key, other
                )));
            }
        }
        match link.transport_security.as_str() {
            "tls_required" | "tls_opportunistic" | "plaintext_legacy" => {}
            other => {
                return Err(CliError::Message(format!(
                    "network link {} has invalid transport_security {}",
                    link.key, other
                )));
            }
        }
    }

    let mut network_area_ids = HashSet::with_capacity(payload.network_areas.len());
    for area in &payload.network_areas {
        if !network_area_ids.insert(area.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate network area id {} in import payload",
                area.id
            )));
        }
        if !network_profile_ids.contains(area.network_id.as_str()) {
            return Err(CliError::Message(format!(
                "network area {} references missing profile {}",
                area.area_tag, area.network_id
            )));
        }
        if !area_ids.contains(area.local_area_id.as_str()) {
            return Err(CliError::Message(format!(
                "network area {} references missing local message area {}",
                area.area_tag, area.local_area_id
            )));
        }
    }

    let mut network_packet_ids = HashSet::with_capacity(payload.network_packets.len());
    for packet in &payload.network_packets {
        if !network_packet_ids.insert(packet.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate network packet id {} in import payload",
                packet.id
            )));
        }
        if !network_profile_ids.contains(packet.network_id.as_str()) {
            return Err(CliError::Message(format!(
                "network packet {} references missing profile {}",
                packet.id, packet.network_id
            )));
        }
        if let Some(link_id) = packet.link_id.as_deref()
            && !network_link_ids.contains(link_id)
        {
            return Err(CliError::Message(format!(
                "network packet {} references missing link {}",
                packet.id, link_id
            )));
        }
    }

    let mut network_message_ids = HashSet::with_capacity(payload.network_messages.len());
    for message in &payload.network_messages {
        if !network_message_ids.insert(message.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate network message id {} in import payload",
                message.id
            )));
        }
        if !network_profile_ids.contains(message.network_id.as_str()) {
            return Err(CliError::Message(format!(
                "network message {} references missing profile {}",
                message.id, message.network_id
            )));
        }
        if let Some(local_message_id) = message.local_message_id.as_deref()
            && !message_ids.contains(local_message_id)
        {
            return Err(CliError::Message(format!(
                "network message {} references missing local message {}",
                message.id, local_message_id
            )));
        }
        if let Some(packet_id) = message.packet_id.as_deref()
            && !network_packet_ids.contains(packet_id)
        {
            return Err(CliError::Message(format!(
                "network message {} references missing packet {}",
                message.id, packet_id
            )));
        }
    }

    for seen_by in &payload.network_seen_by {
        if !network_message_ids.contains(seen_by.message_id.as_str()) {
            return Err(CliError::Message(format!(
                "network seen-by row {} references missing message {}",
                seen_by.id, seen_by.message_id
            )));
        }
        if !network_profile_ids.contains(seen_by.network_id.as_str()) {
            return Err(CliError::Message(format!(
                "network seen-by row {} references missing profile {}",
                seen_by.id, seen_by.network_id
            )));
        }
    }

    for path in &payload.network_path {
        if !network_message_ids.contains(path.message_id.as_str()) {
            return Err(CliError::Message(format!(
                "network path row {} references missing message {}",
                path.id, path.message_id
            )));
        }
        if !network_profile_ids.contains(path.network_id.as_str()) {
            return Err(CliError::Message(format!(
                "network path row {} references missing profile {}",
                path.id, path.network_id
            )));
        }
    }

    for duplicate in &payload.network_duplicate_log {
        if !network_profile_ids.contains(duplicate.network_id.as_str()) {
            return Err(CliError::Message(format!(
                "network duplicate row {} references missing profile {}",
                duplicate.id, duplicate.network_id
            )));
        }
    }

    for poll in &payload.network_poll_log {
        if !network_link_ids.contains(poll.link_id.as_str()) {
            return Err(CliError::Message(format!(
                "network poll row {} references missing link {}",
                poll.id, poll.link_id
            )));
        }
    }

    for subscription in &payload.network_area_subscriptions {
        if !network_area_ids.contains(subscription.area_id.as_str()) {
            return Err(CliError::Message(format!(
                "network subscription {} references missing area {}",
                subscription.id, subscription.area_id
            )));
        }
        if !network_link_ids.contains(subscription.link_id.as_str()) {
            return Err(CliError::Message(format!(
                "network subscription {} references missing link {}",
                subscription.id, subscription.link_id
            )));
        }
    }

    for entry in &payload.network_nodelist {
        if !network_profile_ids.contains(entry.network_id.as_str()) {
            return Err(CliError::Message(format!(
                "network nodelist row {} references missing profile {}",
                entry.id, entry.network_id
            )));
        }
    }

    let mut oxidenet_application_ids = HashSet::with_capacity(payload.oxidenet_applications.len());
    let mut oxidenet_assigned_addresses = HashSet::new();
    for application in &payload.oxidenet_applications {
        if !oxidenet_application_ids.insert(application.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate OxideNet application id {} in import payload",
                application.id
            )));
        }
        if !valid_oxidenet_application_status(&application.status) {
            return Err(CliError::Message(format!(
                "OxideNet application {} has invalid status {}",
                application.id, application.status
            )));
        }
        if application.binkp_port <= 0 || application.binkp_port > 65_535 {
            return Err(CliError::Message(format!(
                "OxideNet application {} has invalid BinkP port {}",
                application.id, application.binkp_port
            )));
        }
        if let Some(telnet_port) = application.telnet_port
            && (telnet_port <= 0 || telnet_port > 65_535)
        {
            return Err(CliError::Message(format!(
                "OxideNet application {} has invalid telnet port {}",
                application.id, telnet_port
            )));
        }
        if let Some(user_id) = application.applicant_user_id.as_deref()
            && !user_ids.contains(user_id)
        {
            return Err(CliError::Message(format!(
                "OxideNet application {} references missing applicant {}",
                application.id, user_id
            )));
        }
        if let Some(user_id) = application.reviewed_by_user_id.as_deref()
            && !user_ids.contains(user_id)
        {
            return Err(CliError::Message(format!(
                "OxideNet application {} references missing reviewer {}",
                application.id, user_id
            )));
        }
        if let Some(address) = application.assigned_address.as_deref()
            && !oxidenet_assigned_addresses.insert(address)
        {
            return Err(CliError::Message(format!(
                "duplicate OxideNet assigned address {} in import payload",
                address
            )));
        }
    }

    let mut oxidenet_node_ids = HashSet::with_capacity(payload.oxidenet_nodes.len());
    let mut oxidenet_node_addresses = HashSet::with_capacity(payload.oxidenet_nodes.len());
    for node in &payload.oxidenet_nodes {
        if !oxidenet_node_ids.insert(node.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate OxideNet node id {} in import payload",
                node.id
            )));
        }
        if !oxidenet_node_addresses.insert(node.address.clone()) {
            return Err(CliError::Message(format!(
                "duplicate OxideNet node address {} in import payload",
                node.address
            )));
        }
        if let Some(application_id) = node.application_id.as_deref()
            && !oxidenet_application_ids.contains(application_id)
        {
            return Err(CliError::Message(format!(
                "OxideNet node {} references missing application {}",
                node.id, application_id
            )));
        }
        if !valid_oxidenet_node_status(&node.status) {
            return Err(CliError::Message(format!(
                "OxideNet node {} has invalid status {}",
                node.id, node.status
            )));
        }
        if node.zone <= 0 || node.net <= 0 || node.node <= 0 || node.point < 0 {
            return Err(CliError::Message(format!(
                "OxideNet node {} has invalid address parts",
                node.id
            )));
        }
        if node.binkp_port <= 0 || node.binkp_port > 65_535 {
            return Err(CliError::Message(format!(
                "OxideNet node {} has invalid BinkP port {}",
                node.id, node.binkp_port
            )));
        }
        if let Some(telnet_port) = node.telnet_port
            && (telnet_port <= 0 || telnet_port > 65_535)
        {
            return Err(CliError::Message(format!(
                "OxideNet node {} has invalid telnet port {}",
                node.id, telnet_port
            )));
        }
    }

    let mut oxidenet_credential_ids = HashSet::with_capacity(payload.oxidenet_credentials.len());
    for credential in &payload.oxidenet_credentials {
        if !oxidenet_credential_ids.insert(credential.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate OxideNet credential id {} in import payload",
                credential.id
            )));
        }
        if !oxidenet_node_ids.contains(credential.node_id.as_str()) {
            return Err(CliError::Message(format!(
                "OxideNet credential {} references missing node {}",
                credential.id, credential.node_id
            )));
        }
        if credential.credential_kind != "binkp_session"
            && credential.credential_kind != "invite_token"
        {
            return Err(CliError::Message(format!(
                "OxideNet credential {} has invalid kind {}",
                credential.id, credential.credential_kind
            )));
        }
        if !valid_oxidenet_credential_status(&credential.status) {
            return Err(CliError::Message(format!(
                "OxideNet credential {} has invalid status {}",
                credential.id, credential.status
            )));
        }
        if credential.secret_hash.trim().is_empty() {
            return Err(CliError::Message(format!(
                "OxideNet credential {} has blank secret_hash",
                credential.id
            )));
        }
    }

    for session in &payload.sessions {
        if let Some(user_id) = session.user_id.as_deref()
            && !user_ids.contains(user_id)
        {
            return Err(CliError::Message(format!(
                "session {} references missing user {}",
                session.id, user_id
            )));
        }
        if session.transport != "telnet" {
            return Err(CliError::Message(format!(
                "session {} has unsupported transport {}",
                session.id, session.transport
            )));
        }
    }

    let mut door_ids = HashSet::with_capacity(payload.doors.len());
    for door in &payload.doors {
        if !door_ids.insert(door.id.clone()) {
            return Err(CliError::Message(format!(
                "duplicate door id {} in import payload",
                door.id
            )));
        }
        if door.time_limit_minutes <= 0 {
            return Err(CliError::Message(format!(
                "door {} has invalid time limit {}",
                door.key, door.time_limit_minutes
            )));
        }
    }

    for run in &payload.door_runs {
        if !door_ids.contains(run.door_id.as_str()) {
            return Err(CliError::Message(format!(
                "door run {} references missing door {}",
                run.id, run.door_id
            )));
        }
        if !user_ids.contains(run.user_id.as_str()) {
            return Err(CliError::Message(format!(
                "door run {} references missing user {}",
                run.id, run.user_id
            )));
        }
        if run.node_number <= 0 {
            return Err(CliError::Message(format!(
                "door run {} has invalid node number {}",
                run.id, run.node_number
            )));
        }
        if run.bytes_in < 0 {
            return Err(CliError::Message(format!(
                "door run {} has invalid bytes_in {}",
                run.id, run.bytes_in
            )));
        }
        if run.bytes_out < 0 {
            return Err(CliError::Message(format!(
                "door run {} has invalid bytes_out {}",
                run.id, run.bytes_out
            )));
        }
    }

    for event in &payload.audit_events {
        if let Some(user_id) = event.user_id.as_deref()
            && !user_ids.contains(user_id)
        {
            return Err(CliError::Message(format!(
                "audit event {} references missing user {}",
                event.id, user_id
            )));
        }
    }

    Ok(())
}

fn valid_oxidenet_application_status(status: &str) -> bool {
    matches!(
        status,
        "draft"
            | "submitted"
            | "needs-info"
            | "approved"
            | "config-generated"
            | "first-poll-pending"
            | "active"
            | "probation"
            | "suspended"
            | "retired"
            | "rejected"
            | "withdrawn"
            | "needs-review-hold"
    )
}

fn valid_oxidenet_node_status(status: &str) -> bool {
    matches!(
        status,
        "config-generated"
            | "first-poll-pending"
            | "active"
            | "probation"
            | "suspended"
            | "retired"
    )
}

fn valid_oxidenet_credential_status(status: &str) -> bool {
    matches!(status, "active" | "revoked" | "expired")
}

fn insert_messages_with_replies(db: &Db, messages: &[MessageRecord]) -> CliResult<()> {
    for message in messages {
        let mut without_reply = message.clone();
        without_reply.reply_to_id = None;
        insert_message(db, &without_reply)?;
    }

    for message in messages {
        if let Some(reply_to_id) = &message.reply_to_id {
            db.execute_with_params(
                "UPDATE messages SET reply_to_id = UUID_PARSE($1) WHERE id = UUID_PARSE($2)",
                &[
                    Value::Text(reply_to_id.clone()),
                    Value::Text(message.id.clone()),
                ],
            )?;
        }
    }
    Ok(())
}

fn perform_db_import(db: &oxidebbs_db::OxideDb, payload: ImportSchema) -> CliResult<()> {
    let current_schema = read_schema_version(db.db())?;
    validate_import_payload(&payload, current_schema)?;
    ensure_import_target_is_schema_only(db.db())?;

    let users: Vec<UserRecord> = payload.users.into_iter().map(Into::into).collect();
    let auth_attempts: Vec<AuthAttemptRecord> =
        payload.auth_attempts.into_iter().map(Into::into).collect();
    let message_areas: Vec<MessageAreaRecord> =
        payload.message_areas.into_iter().map(Into::into).collect();
    let messages: Vec<MessageRecord> = payload.messages.into_iter().map(Into::into).collect();
    let sessions: Vec<SessionRecord> = payload.sessions.into_iter().map(Into::into).collect();
    let doors: Vec<DoorDefinitionRecord> = payload.doors.into_iter().map(Into::into).collect();
    let door_runs: Vec<DoorRunRecord> = payload.door_runs.into_iter().map(Into::into).collect();
    let door_provider_credentials: Vec<DoorProviderCredentialRecord> = payload
        .door_provider_credentials
        .into_iter()
        .filter(|c| c.credential_ref != "[redacted]")
        .map(Into::into)
        .collect();
    let audit_events: Vec<AuditEventRecord> =
        payload.audit_events.into_iter().map(Into::into).collect();
    let network_profiles: Vec<NetworkProfileRecord> = payload
        .network_profiles
        .into_iter()
        .map(Into::into)
        .collect();
    let network_links: Vec<NetworkLinkRecord> =
        payload.network_links.into_iter().map(Into::into).collect();
    let network_areas: Vec<NetworkAreaRecord> =
        payload.network_areas.into_iter().map(Into::into).collect();
    let network_packets: Vec<NetworkPacketRecord> = payload
        .network_packets
        .into_iter()
        .map(Into::into)
        .collect();
    let network_messages: Vec<NetworkMessageRecord> = payload
        .network_messages
        .into_iter()
        .map(Into::into)
        .collect();
    let network_seen_by: Vec<NetworkSeenByNode> = payload
        .network_seen_by
        .into_iter()
        .map(Into::into)
        .collect();
    let network_path: Vec<NetworkPathNode> =
        payload.network_path.into_iter().map(Into::into).collect();
    let network_duplicates: Vec<NetworkDuplicateLogRecord> = payload
        .network_duplicate_log
        .into_iter()
        .map(Into::into)
        .collect();
    let network_poll_logs: Vec<NetworkPollLogRecord> = payload
        .network_poll_log
        .into_iter()
        .map(Into::into)
        .collect();
    let network_subscriptions: Vec<NetworkSubscriptionRecord> = payload
        .network_area_subscriptions
        .into_iter()
        .map(Into::into)
        .collect();
    let network_nodelist: Vec<NetworkNodelistRecord> = payload
        .network_nodelist
        .into_iter()
        .map(Into::into)
        .collect();
    let oxidenet_applications: Vec<OxideNetApplicationRecord> = payload
        .oxidenet_applications
        .into_iter()
        .map(Into::into)
        .collect();
    let oxidenet_nodes: Vec<OxideNetNodeRecord> =
        payload.oxidenet_nodes.into_iter().map(Into::into).collect();
    let oxidenet_credentials: Vec<OxideNetCredentialRecord> = payload
        .oxidenet_credentials
        .into_iter()
        .map(Into::into)
        .collect();

    let db = db.db();
    db.begin_transaction()?;
    let result = (|| -> CliResult<()> {
        for user in &users {
            insert_user(db, user)?;
        }
        for attempt in &auth_attempts {
            insert_auth_attempt(db, attempt)?;
        }
        for area in &message_areas {
            insert_message_area(db, area)?;
        }
        insert_messages_with_replies(db, &messages)?;
        for profile in &network_profiles {
            insert_network_profile(db, profile)?;
        }
        for link in &network_links {
            insert_network_link(db, link)?;
        }
        for area in &network_areas {
            insert_network_area(db, area)?;
        }
        for packet in &network_packets {
            insert_network_packet(db, packet)?;
        }
        for message in &network_messages {
            insert_network_message(db, message)?;
        }
        for seen_by in &network_seen_by {
            insert_network_seen_by(db, seen_by)?;
        }
        for path in &network_path {
            insert_network_path_node(db, path)?;
        }
        for duplicate in &network_duplicates {
            insert_network_duplicate_log(db, duplicate)?;
        }
        for poll in &network_poll_logs {
            insert_network_poll_log(db, poll)?;
        }
        for subscription in &network_subscriptions {
            insert_network_subscription(db, subscription)?;
        }
        for entry in &network_nodelist {
            insert_network_nodelist_entry(db, entry)?;
        }
        for application in &oxidenet_applications {
            insert_oxidenet_application(db, application)?;
        }
        for node in &oxidenet_nodes {
            insert_oxidenet_node(db, node)?;
        }
        for credential in &oxidenet_credentials {
            insert_oxidenet_credential(db, credential)?;
        }
        for session in &sessions {
            insert_session(db, session)?;
        }
        for door in &doors {
            insert_door_definition(db, door)?;
        }
        for run in &door_runs {
            insert_door_run(db, run)?;
        }
        for credential in &door_provider_credentials {
            insert_door_provider_credential(db, credential)?;
        }
        for event in &audit_events {
            insert_audit_event_preserving_record(db, event)?;
        }
        Ok(())
    })();

    match result {
        Ok(()) => {
            db.commit_transaction()?;
        }
        Err(error) => {
            let _ = db.rollback_transaction();
            return Err(error);
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct DatabaseVerifyReport {
    pass: usize,
    warn: usize,
    fail: usize,
    results: Vec<serde_json::Value>,
}

fn verify_database(db: &oxidebbs_db::OxideDb) -> DatabaseVerifyReport {
    let mut pass = 0usize;
    let warn = 0usize;
    let mut fail = 0usize;
    let mut results: Vec<serde_json::Value> = Vec::new();

    match db.schema_version() {
        Ok(schema_version) => {
            let schema_ok = schema_version == oxidebbs_db::SCHEMA_VERSION;
            if schema_ok {
                pass += 1;
                results.push(serde_json::json!({"check": "schema_version", "status": "pass", "value": schema_version}));
            } else {
                fail += 1;
                results.push(serde_json::json!({"check": "schema_version", "status": "fail", "expected": oxidebbs_db::SCHEMA_VERSION, "actual": schema_version}));
            }
        }
        Err(error) => {
            fail += 1;
            results.push(serde_json::json!({"check": "schema_version", "status": "fail", "error": error.to_string()}));
        }
    }

    let tables = [
        "users",
        "auth_attempts",
        "message_areas",
        "messages",
        "sessions",
        "doors",
        "door_runs",
        "audit_events",
        "network_profiles",
        "network_links",
        "network_areas",
        "network_packets",
        "network_messages",
        "network_seen_by",
        "network_path",
        "network_duplicate_log",
        "network_poll_log",
        "network_area_subscriptions",
        "network_nodelist",
        "network_applications",
        "network_nodes",
        "network_credentials",
        "file_areas",
        "file_entries",
        "file_transfers",
    ];
    for table in &tables {
        match db.db().execute(&format!("SELECT COUNT(*) FROM {table}")) {
            Ok(_) => {
                pass += 1;
                results.push(
                    serde_json::json!({"check": format!("table_exists:{table}"), "status": "pass"}),
                );
            }
            Err(error) => {
                fail += 1;
                results.push(serde_json::json!({"check": format!("table_exists:{table}"), "status": "fail", "error": error.to_string()}));
            }
        }
    }

    for name in [
        "users",
        "messages",
        "doors",
        "sessions",
        "file_areas",
        "oxidenet_nodes",
    ] {
        let result: Result<(), String> = match name {
            "users" => oxidebbs_db::list_users(db.db())
                .map(|_| ())
                .map_err(|error| error.to_string()),
            "messages" => oxidebbs_db::list_messages(db.db())
                .map(|_| ())
                .map_err(|error| error.to_string()),
            "doors" => oxidebbs_db::list_door_definitions(db.db())
                .map(|_| ())
                .map_err(|error| error.to_string()),
            "sessions" => oxidebbs_db::list_recent_sessions(db.db(), 1)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            "file_areas" => oxidebbs_db::list_file_areas(db.db())
                .map(|_| ())
                .map_err(|error| error.to_string()),
            "oxidenet_nodes" => oxidebbs_db::list_oxidenet_nodes(db.db(), 1)
                .map(|_| ())
                .map_err(|error| error.to_string()),
            _ => Ok(()),
        };

        match result {
            Ok(()) => {
                pass += 1;
                results.push(
                    serde_json::json!({"check": format!("repo_read:{name}"), "status": "pass"}),
                );
            }
            Err(error) => {
                fail += 1;
                results.push(serde_json::json!({"check": format!("repo_read:{name}"), "status": "fail", "error": error}));
            }
        }
    }

    DatabaseVerifyReport {
        pass,
        warn,
        fail,
        results,
    }
}

fn emit_verify_report(
    ctx: &AppContext,
    path: &Path,
    report: &DatabaseVerifyReport,
) -> CliResult<()> {
    if ctx.json {
        print_json(&serde_json::json!({
            "ok": report.fail == 0,
            "pass": report.pass,
            "warn": report.warn,
            "fail": report.fail,
            "results": report.results
        }))?;
    } else {
        println!("database verify: {}", path.display());
        for result in &report.results {
            let check = result["check"].as_str().unwrap_or("?");
            let status = result["status"].as_str().unwrap_or("?");
            let error = result
                .get("error")
                .and_then(|value| value.as_str())
                .unwrap_or("");
            let extra = if !error.is_empty() {
                format!(" - {error}")
            } else {
                String::new()
            };
            println!("  [{status}] {check}{extra}");
        }
        println!(
            "\npass: {}, warn: {}, fail: {}",
            report.pass, report.warn, report.fail
        );
    }

    if report.fail > 0 {
        return Err(CliError::Message(format!(
            "database verify failed with {} failures",
            report.fail
        )));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct CompactResult {
    source: PathBuf,
    output: PathBuf,
    source_bytes: u64,
    output_bytes: u64,
    schema_version: i64,
    verify_passes: usize,
}

fn db_compact(source_path: &Path, output_path: &Path, overwrite: bool) -> CliResult<CompactResult> {
    if !source_path.exists() {
        return Err(CliError::Message(format!(
            "database file {} does not exist",
            source_path.display()
        )));
    }

    if let Some(parent) = output_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }

    reject_compact_output_source_collision(source_path, output_path)?;

    if output_path.exists() {
        if !overwrite {
            return Err(CliError::Message(format!(
                "compact output {} already exists; pass --overwrite to replace it",
                output_path.display()
            )));
        }
        fs::remove_file(output_path)?;
    }

    let db = oxidebbs_db::OxideDb::open_or_create(source_path)?;
    let source_bytes = fs::metadata(source_path)?.len();
    db.db().checkpoint_wal()?;
    db.db().save_as(output_path)?;
    evict_shared_wal(output_path)?;
    drop(db);

    let compacted = oxidebbs_db::OxideDb::open_or_create(output_path)?;
    let report = verify_database(&compacted);
    if report.fail > 0 {
        return Err(CliError::Message(format!(
            "compacted database {} failed verification with {} failures",
            output_path.display(),
            report.fail
        )));
    }

    Ok(CompactResult {
        source: source_path.to_path_buf(),
        output: output_path.to_path_buf(),
        source_bytes,
        output_bytes: fs::metadata(output_path)?.len(),
        schema_version: compacted.schema_version()?,
        verify_passes: report.pass,
    })
}

fn reject_compact_output_source_collision(source_path: &Path, output_path: &Path) -> CliResult<()> {
    let source = fs::canonicalize(source_path)?;
    let output = if output_path.exists() {
        fs::canonicalize(output_path)?
    } else {
        let parent = output_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let file_name = output_path.file_name().ok_or_else(|| {
            CliError::Message(format!(
                "compact output path {} must name a database file",
                output_path.display()
            ))
        })?;
        fs::canonicalize(parent)?.join(file_name)
    };

    if source == output {
        return Err(CliError::Message(
            "compact output must not be the active database path".to_string(),
        ));
    }

    Ok(())
}

#[derive(Subcommand)]
// Lifecycle order keeps admin verbs grouped by maintenance workflow, with `verify` after restore primitives.
pub enum DbCommand {
    Init,
    Doctor,
    Stats,
    Backup {
        output_path: std::path::PathBuf,
    },
    Export {
        #[arg(long, default_value = "json")]
        format: String,
    },
    Import {
        #[arg(long, default_value = "json")]
        format: String,
        path: std::path::PathBuf,
    },
    Compact {
        #[arg(long)]
        output: std::path::PathBuf,
        #[arg(long, default_value_t = false)]
        overwrite: bool,
    },
    Verify,
}

pub fn run_db(command: DbCommand, ctx: &AppContext) -> CliResult<()> {
    match command {
        DbCommand::Init => {
            let db = open_database(&ctx.config)?;
            emit_ok(
                ctx.json,
                "database initialized",
                JsonValue::Object(
                    serde_json::json!({
                        "path": ctx.config.database.path,
                        "schema_version": db.schema_version()?
                    })
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
                ),
            )
        }
        DbCommand::Doctor => {
            let db = open_database(&ctx.config)?;
            let version = db.schema_version()?;
            let stats = db_stats(db.db(), live_active_session_count(ctx)?)?;
            if ctx.json {
                print_json(
                    &serde_json::json!({"ok": true, "schema_version": version, "stats": stats}),
                )?;
            } else {
                println!("database OK: {}", ctx.config.database.path.display());
                println!("schema version: {version}");
                print_stats(&stats);
            }
            Ok(())
        }
        DbCommand::Verify => {
            let db = open_database(&ctx.config)?;
            let report = verify_database(&db);
            emit_verify_report(ctx, &ctx.config.database.path, &report)
        }
        DbCommand::Stats => {
            let db = open_database(&ctx.config)?;
            let stats = db_stats(db.db(), live_active_session_count(ctx)?)?;
            if ctx.json {
                print_json(&stats)?;
            } else {
                print_stats(&stats);
            }
            Ok(())
        }
        DbCommand::Backup { output_path } => {
            let source = &ctx.config.database.path;
            if !source.exists() {
                return Err(CliError::Message(format!(
                    "database file {} does not exist",
                    source.display()
                )));
            }
            if let Some(parent) = output_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source, &output_path)?;
            emit_ok(
                ctx.json,
                "database backup complete",
                serde_json::json!({"output": output_path}),
            )
        }
        DbCommand::Export { format } => {
            require_json_format(&format)?;
            let db = open_database(&ctx.config)?;
            print_json(&db_export(db.db())?)?;
            Ok(())
        }
        DbCommand::Import { format, path } => {
            let payload = parse_db_import(&path, &format)?;
            let db = open_database(&ctx.config)?;
            let import_counts = (
                payload.users.len(),
                payload.auth_attempts.len(),
                payload.message_areas.len(),
                payload.messages.len(),
                payload.sessions.len(),
                payload.doors.len(),
                payload.door_runs.len(),
                payload.audit_events.len(),
            );
            perform_db_import(&db, payload)?;
            emit_ok(
                ctx.json,
                "database imported",
                serde_json::json!({
                    "schema_version": db.schema_version()?,
                    "users": import_counts.0,
                    "auth_attempts": import_counts.1,
                    "message_areas": import_counts.2,
                    "messages": import_counts.3,
                    "sessions": import_counts.4,
                    "doors": import_counts.5,
                    "door_runs": import_counts.6,
                    "audit_events": import_counts.7,
                }),
            )
        }
        DbCommand::Compact { output, overwrite } => {
            let result = db_compact(&ctx.config.database.path, &output, overwrite)?;
            emit_ok(
                ctx.json,
                "database compact complete",
                serde_json::json!({
                    "source": result.source,
                    "output": result.output,
                    "source_bytes": result.source_bytes,
                    "output_bytes": result.output_bytes,
                    "schema_version": result.schema_version,
                    "verify_passes": result.verify_passes,
                    "in_place": false
                }),
            )
        }
    }
}

fn require_json_format(format: &str) -> CliResult<()> {
    if format.eq_ignore_ascii_case("json") {
        Ok(())
    } else {
        Err(CliError::Message(format!(
            "unsupported format {format:?}; only json is supported"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxidebbs_db::{
        SCHEMA_VERSION, find_message_by_id, insert_audit_event, insert_door_definition,
        insert_door_provider_credential, insert_door_run, insert_message, insert_message_area,
        insert_oxidenet_application, insert_oxidenet_credential, insert_oxidenet_node,
        insert_session, insert_user, list_door_provider_credentials, list_oxidenet_applications,
        list_oxidenet_credentials_for_node, list_oxidenet_nodes, list_users,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_path(tag: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "oxidebbs-phase5-{tag}-{}.json",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        path
    }

    fn make_temp_db_path(tag: &str) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "oxidebbs-phase6-{tag}-{}.ddb",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system time should be valid")
                .as_nanos()
        ));
        path
    }

    fn test_db() -> oxidebbs_db::OxideDb {
        let db = oxidebbs_db::OxideDb::open_memory().expect("open in-memory DB");
        assert_eq!(db.schema_version().expect("schema version"), SCHEMA_VERSION);
        db
    }

    fn seed_user(id: &str, alias: &str, is_sysop: bool) -> UserRecord {
        UserRecord {
            id: id.to_string(),
            alias: alias.to_string(),
            real_name: format!("{alias} User"),
            email: Some(format!("{alias}@example.com")),
            password_hash: "hash".to_string(),
            security_level: 10,
            is_sysop,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            last_login_at: None,
            total_calls: 0,
            time_bank_minutes: 0,
            status: "active".to_string(),
        }
    }

    fn seed_area(id: &str) -> MessageAreaRecord {
        MessageAreaRecord {
            id: id.to_string(),
            key: if id.ends_with("101") {
                "general".to_string()
            } else {
                "games".to_string()
            },
            name: "General".to_string(),
            description: "desc".to_string(),
            kind: "local".to_string(),
            network_id: None,
            read_security_level: 0,
            post_security_level: 10,
            moderated: false,
            enabled: true,
        }
    }

    fn seed_message(
        id: &str,
        area_id: &str,
        author_id: &str,
        reply_to_id: Option<&str>,
    ) -> MessageRecord {
        MessageRecord {
            id: id.to_string(),
            area_id: area_id.to_string(),
            author_user_id: author_id.to_string(),
            author_kind: "local".to_string(),
            author_display_name: String::new(),
            author_network_address: None,
            to_user_id: None,
            subject: "Hello".to_string(),
            body: "Body".to_string(),
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            reply_to_id: reply_to_id.map(std::string::ToString::to_string),
            network_message_id: None,
            visibility: "normal".to_string(),
        }
    }

    fn seed_oxidenet_application(id: &str) -> OxideNetApplicationRecord {
        OxideNetApplicationRecord {
            id: id.to_string(),
            created_at: "2026-06-04T00:00:00.000000Z".to_string(),
            updated_at: "2026-06-04T00:00:00.000000Z".to_string(),
            submitted_at: Some("2026-06-04T00:00:00.000000Z".to_string()),
            reviewed_at: Some("2026-06-04T01:00:00.000000Z".to_string()),
            status: "approved".to_string(),
            applicant_user_id: None,
            board_name: "Example BBS".to_string(),
            sysop_alias: "Sysop".to_string(),
            contact_email: "sysop@example.test".to_string(),
            host: "bbs.example.test".to_string(),
            binkp_port: 24554,
            telnet_host: Some("bbs.example.test".to_string()),
            telnet_port: Some(23),
            software: "OxideBBS".to_string(),
            software_version: "1.3.0".to_string(),
            timezone: "America/Chicago".to_string(),
            region: "NA".to_string(),
            description: "test board".to_string(),
            reason: "join oxidenet".to_string(),
            policy_version: "2026-06-04".to_string(),
            policy_accepted_at: Some("2026-06-04T00:00:00.000000Z".to_string()),
            admin_notes: "approved".to_string(),
            reviewed_by_user_id: None,
            assigned_address: Some("777:1/100".to_string()),
        }
    }

    fn seed_oxidenet_node(id: &str, application_id: &str) -> OxideNetNodeRecord {
        OxideNetNodeRecord {
            id: id.to_string(),
            application_id: Some(application_id.to_string()),
            network_key: "oxidenet".to_string(),
            address: "777:1/100".to_string(),
            zone: 777,
            net: 1,
            node: 100,
            point: 0,
            hub_address: "777:1/1".to_string(),
            board_name: "Example BBS".to_string(),
            sysop_alias: "Sysop".to_string(),
            contact_email: "sysop@example.test".to_string(),
            host: "bbs.example.test".to_string(),
            binkp_port: 24554,
            telnet_host: None,
            telnet_port: None,
            software: "OxideBBS".to_string(),
            software_version: "1.3.0".to_string(),
            status: "active".to_string(),
            created_at: "2026-06-04T01:00:00.000000Z".to_string(),
            updated_at: "2026-06-04T01:00:00.000000Z".to_string(),
            activated_at: Some("2026-06-04T01:30:00.000000Z".to_string()),
            suspended_at: None,
            retired_at: None,
            last_poll_at: Some("2026-06-04T02:00:00.000000Z".to_string()),
            last_successful_poll_at: Some("2026-06-04T02:00:00.000000Z".to_string()),
            flags: "CM".to_string(),
        }
    }

    fn seed_oxidenet_credential(id: &str, node_id: &str) -> OxideNetCredentialRecord {
        OxideNetCredentialRecord {
            id: id.to_string(),
            node_id: node_id.to_string(),
            credential_kind: "binkp_session".to_string(),
            secret_hash: "sha256:abc123".to_string(),
            created_at: "2026-06-04T01:00:00.000000Z".to_string(),
            rotated_at: None,
            expires_at: None,
            status: "active".to_string(),
        }
    }

    fn table_counts(db: &oxidebbs_db::OxideDb) -> (i64, i64, i64, i64, i64, i64, i64, i64) {
        (
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM users").expect("count users"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM auth_attempts")
                .expect("count auth attempts"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM message_areas").expect("count areas"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM messages").expect("count messages"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM sessions").expect("count sessions"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM doors").expect("count doors"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM door_runs").expect("count runs"),
            db_scalar_i64(db.db(), "SELECT COUNT(*) FROM audit_events").expect("count events"),
        )
    }

    fn export_payload(db: &oxidebbs_db::OxideDb) -> ImportSchema {
        let value = db_export(db.db()).expect("export");
        serde_json::from_value(value).expect("parse export payload")
    }

    fn seeded_source_db() -> (oxidebbs_db::OxideDb, ImportSchema) {
        let source = test_db();
        let user = seed_user("00000000-0000-4000-8000-000000000001", "alice", true);
        let area = seed_area("00000000-0000-4000-8000-000000000101");
        let root_message = seed_message(
            "00000000-0000-4000-8000-000000000201",
            &area.id,
            &user.id,
            None,
        );
        let reply_message = seed_message(
            "00000000-0000-4000-8000-000000000202",
            &area.id,
            &user.id,
            Some(&root_message.id),
        );
        let session = SessionRecord {
            id: "00000000-0000-4000-8000-000000000301".to_string(),
            node_number: 1,
            user_id: Some(user.id.clone()),
            transport: "telnet".to_string(),
            remote_address: "127.0.0.1:2323".to_string(),
            remote_ip: Some("127.0.0.1".to_string()),
            remote_port: Some(2323),
            started_at: "2026-01-01T00:00:00.000000Z".to_string(),
            ended_at: None,
            disconnect_reason: None,
        };
        let door = DoorDefinitionRecord {
            id: "00000000-0000-4000-8000-000000000401".to_string(),
            key: "echo".to_string(),
            name: "Echo".to_string(),
            runner: "dosemu".to_string(),
            working_dir: "/tmp".to_string(),
            command: "ECHO.EXE".to_string(),
            drop_file: "DOOR.SYS".to_string(),
            exclusive: false,
            time_limit_minutes: 30,
            enabled: true,
            min_security_level: 0,
        };
        let door_run = DoorRunRecord {
            id: "00000000-0000-4000-8000-000000000501".to_string(),
            door_id: door.id.clone(),
            user_id: user.id.clone(),
            node_number: 1,
            started_at: "2026-01-01T00:10:00.000000Z".to_string(),
            ended_at: Some("2026-01-01T00:20:00.000000Z".to_string()),
            exit_code: Some(0),
            timed_out: false,
            disconnect_forced: false,
            bytes_in: 1,
            bytes_out: 2,
        };
        let audit_event = ImportAuditEventRecord {
            id: "00000000-0000-4000-8000-000000000601".to_string(),
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            event_type: "import_fixture".to_string(),
            user_id: Some(user.id.clone()),
            node_number: Some(1),
            details: "created fixture".to_string(),
        };
        let oxidenet_application =
            seed_oxidenet_application("00000000-0000-4000-8000-000000000701");
        let oxidenet_node = seed_oxidenet_node(
            "00000000-0000-4000-8000-000000000702",
            &oxidenet_application.id,
        );
        let oxidenet_credential =
            seed_oxidenet_credential("00000000-0000-4000-8000-000000000703", &oxidenet_node.id);

        insert_user(source.db(), &user).expect("seed user");
        insert_message_area(source.db(), &area).expect("seed area");
        insert_message(source.db(), &root_message).expect("seed root message");
        insert_message(source.db(), &reply_message).expect("seed reply");
        insert_session(source.db(), &session).expect("seed session");
        insert_door_definition(source.db(), &door).expect("seed door");
        insert_door_run(source.db(), &door_run).expect("seed run");
        insert_audit_event(source.db(), &audit_event.into()).expect("seed audit");
        insert_oxidenet_application(source.db(), &oxidenet_application)
            .expect("seed oxidenet application");
        insert_oxidenet_node(source.db(), &oxidenet_node).expect("seed oxidenet node");
        insert_oxidenet_credential(source.db(), &oxidenet_credential)
            .expect("seed oxidenet credential");

        let payload = export_payload(&source);
        (source, payload)
    }

    #[test]
    fn db_init_target_is_schema_only_for_import() {
        let db = test_db();
        assert_eq!(table_counts(&db), (0, 0, 0, 0, 0, 0, 0, 0));
        ensure_import_target_is_schema_only(db.db()).expect("schema-only target");
    }

    #[test]
    fn import_restores_schema_only_target() {
        let (_, payload) = seeded_source_db();
        let target = test_db();
        perform_db_import(&target, payload).expect("import");
        assert_eq!(table_counts(&target), (1, 0, 1, 2, 1, 1, 1, 1));
        let users = list_users(target.db()).expect("list users");
        assert_eq!(users[0].alias, "alice");
        assert_eq!(users[0].password_hash, "hash");
        let reply = find_message_by_id(target.db(), "00000000-0000-4000-8000-000000000202")
            .expect("find reply")
            .expect("reply exists");
        assert_eq!(
            reply.reply_to_id.as_deref(),
            Some("00000000-0000-4000-8000-000000000201")
        );
        let applications = list_oxidenet_applications(target.db(), 10).expect("list applications");
        let nodes = list_oxidenet_nodes(target.db(), 10).expect("list nodes");
        let credentials =
            list_oxidenet_credentials_for_node(target.db(), &nodes[0].id).expect("list creds");
        assert_eq!(applications.len(), 1);
        assert_eq!(
            applications[0].assigned_address.as_deref(),
            Some("777:1/100")
        );
        assert_eq!(nodes.len(), 1);
        assert_eq!(
            nodes[0].application_id.as_deref(),
            Some(applications[0].id.as_str())
        );
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].secret_hash, "sha256:abc123");
    }

    #[test]
    fn db_export_redacts_door_provider_credentials_and_import_skips_redacted_refs() {
        let source = test_db();
        let door = DoorDefinitionRecord {
            id: "00000000-0000-4000-8000-000000000401".to_string(),
            key: "bbslink-lord".to_string(),
            name: "Remote LORD".to_string(),
            runner: "remote:bbslink".to_string(),
            working_dir: "telnet://127.0.0.1:2323".to_string(),
            command: "LORD".to_string(),
            drop_file: "DOOR.SYS".to_string(),
            exclusive: false,
            time_limit_minutes: 30,
            enabled: true,
            min_security_level: 0,
        };
        let credential = DoorProviderCredentialRecord {
            id: "00000000-0000-4000-8000-000000000402".to_string(),
            door_id: door.id.clone(),
            provider_name: "bbslink".to_string(),
            credential_ref: "vault://doors/bbslink-lord/auth-code".to_string(),
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000000Z".to_string(),
        };
        insert_door_definition(source.db(), &door).expect("seed door");
        insert_door_provider_credential(source.db(), &credential).expect("seed credential");

        let payload = export_payload(&source);

        assert_eq!(payload.door_provider_credentials.len(), 1);
        assert_eq!(
            payload.door_provider_credentials[0].credential_ref,
            "[redacted]"
        );

        let target = test_db();
        perform_db_import(&target, payload).expect("import redacted payload");

        let credentials =
            list_door_provider_credentials(target.db(), &door.id).expect("list credentials");
        assert!(credentials.is_empty());
    }

    #[test]
    fn import_restores_oxidenet_registry_rows() {
        let source = test_db();
        let application = seed_oxidenet_application("00000000-0000-4000-8000-000000000701");
        let node = seed_oxidenet_node("00000000-0000-4000-8000-000000000702", &application.id);
        let credential = seed_oxidenet_credential("00000000-0000-4000-8000-000000000703", &node.id);

        insert_oxidenet_application(source.db(), &application).expect("seed application");
        insert_oxidenet_node(source.db(), &node).expect("seed node");
        insert_oxidenet_credential(source.db(), &credential).expect("seed credential");

        let payload = export_payload(&source);
        assert_eq!(payload.oxidenet_applications.len(), 1);
        assert_eq!(payload.oxidenet_nodes.len(), 1);
        assert_eq!(payload.oxidenet_credentials.len(), 1);

        let target = test_db();
        perform_db_import(&target, payload).expect("import");

        let applications = list_oxidenet_applications(target.db(), 10).expect("list applications");
        let nodes = list_oxidenet_nodes(target.db(), 10).expect("list nodes");
        let credentials = list_oxidenet_credentials_for_node(target.db(), &nodes[0].id)
            .expect("list credentials");

        assert_eq!(
            applications[0].assigned_address.as_deref(),
            Some("777:1/100")
        );
        assert_eq!(nodes[0].address, "777:1/100");
        assert_eq!(credentials[0].secret_hash, "sha256:abc123");
    }

    #[test]
    fn import_rejects_schema_version_mismatch() {
        let (_, mut payload) = seeded_source_db();
        payload.schema_version = 2;
        let target = test_db();
        let before = table_counts(&target);
        let err = perform_db_import(&target, payload).expect_err("expected schema mismatch");
        let after = table_counts(&target);
        assert!(
            err.to_string()
                .contains("unsupported import schema version")
        );
        assert_eq!(before, after);
    }

    #[test]
    fn import_rejects_populated_target() {
        let (_, payload) = seeded_source_db();
        let target = test_db();
        let seed = seed_user("00000000-0000-4000-8000-000000000777", "sysop", true);
        insert_user(target.db(), &seed).expect("seed target sysop");
        let event = AuditEventRecord {
            id: "00000000-0000-4000-8000-000000000778".to_string(),
            created_at: "2026-01-01T01:00:00.000000Z".to_string(),
            event_type: "seeded".to_string(),
            user_id: Some(seed.id),
            node_number: Some(1),
            details: "seeded".to_string(),
        };
        insert_audit_event(target.db(), &event).expect("seed target audit");
        let before = table_counts(&target);
        let err =
            perform_db_import(&target, payload).expect_err("expected populated target rejection");
        let after = table_counts(&target);
        assert!(
            err.to_string()
                .contains("import target must be schema-only")
        );
        assert_eq!(before, after);
    }

    #[test]
    fn import_rejects_malformed_json() {
        let path = make_temp_path("malformed");
        fs::write(&path, "{bad json").expect("write");
        let err = parse_db_import(&path, "json").expect_err("expected invalid json to fail");
        assert!(!err.to_string().is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn compact_writes_verified_output_database() {
        let source_path = make_temp_db_path("source");
        let output_path = make_temp_db_path("output");
        let _ = fs::remove_file(&source_path);
        let _ = fs::remove_file(&output_path);

        {
            let source =
                oxidebbs_db::OxideDb::open_or_create(&source_path).expect("open source database");
            let user = seed_user("00000000-0000-4000-8000-000000000901", "compact", true);
            insert_user(source.db(), &user).expect("seed source user");
        }

        let result =
            db_compact(&source_path, &output_path, false).expect("compact source database");

        assert_eq!(result.source, source_path);
        assert_eq!(result.output, output_path);
        assert_eq!(result.schema_version, SCHEMA_VERSION);
        assert!(result.source_bytes > 0);
        assert!(result.output_bytes > 0);
        assert!(result.verify_passes > 0);

        let output =
            oxidebbs_db::OxideDb::open_or_create(&output_path).expect("open compacted database");
        let users = list_users(output.db()).expect("list compacted users");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].alias, "compact");

        let _ = fs::remove_file(source_path);
        let _ = fs::remove_file(output_path);
    }

    #[test]
    fn compact_rejects_existing_output_without_overwrite() {
        let source_path = make_temp_db_path("existing-source");
        let output_path = make_temp_db_path("existing-output");
        let _ = fs::remove_file(&source_path);
        let _ = fs::remove_file(&output_path);

        {
            let _source =
                oxidebbs_db::OxideDb::open_or_create(&source_path).expect("open source database");
        }
        fs::write(&output_path, b"existing").expect("write existing output");

        let err = db_compact(&source_path, &output_path, false)
            .expect_err("existing output should require overwrite");
        assert!(err.to_string().contains("already exists"));

        let _ = fs::remove_file(source_path);
        let _ = fs::remove_file(output_path);
    }

    #[test]
    fn compact_overwrites_when_requested() {
        let source_path = make_temp_db_path("overwrite-source");
        let output_path = make_temp_db_path("overwrite-output");
        let _ = fs::remove_file(&source_path);
        let _ = fs::remove_file(&output_path);

        {
            let _source =
                oxidebbs_db::OxideDb::open_or_create(&source_path).expect("open source database");
        }
        fs::write(&output_path, b"existing").expect("write existing output");

        let result =
            db_compact(&source_path, &output_path, true).expect("overwrite compact output");
        assert_eq!(result.output, output_path);
        assert!(result.output_bytes > 0);

        let _ = fs::remove_file(source_path);
        let _ = fs::remove_file(output_path);
    }

    #[test]
    fn compact_rejects_active_database_as_output() {
        let source_path = make_temp_db_path("same-path");
        let _ = fs::remove_file(&source_path);

        {
            let _source =
                oxidebbs_db::OxideDb::open_or_create(&source_path).expect("open source database");
        }

        let err = db_compact(&source_path, &source_path, true)
            .expect_err("active database should not be compact output");
        assert!(err.to_string().contains("active database path"));

        let _ = fs::remove_file(source_path);
    }

    #[test]
    fn db_stats_json_shape_matches_contract() {
        let db = test_db();
        let stats = db_stats(db.db(), 0).expect("stats");
        let obj = stats.as_object().expect("stats object");
        assert!(obj.contains_key("schema_version"));
        assert!(obj.contains_key("users"));
        assert!(obj.contains_key("message_areas"));
        assert!(obj.contains_key("messages"));
        assert!(obj.contains_key("sessions"));
        assert!(obj.contains_key("active_sessions"));
        assert!(obj.contains_key("open_sessions"));
        assert!(obj.contains_key("auth_attempts"));
        assert!(obj.contains_key("doors"));
        assert!(obj.contains_key("door_runs"));
        assert!(obj.contains_key("audit_events"));
    }

    #[test]
    fn db_stats_distinguishes_live_active_from_open_session_rows() {
        let db = test_db();
        let user = seed_user("00000000-0000-4000-8000-000000000701", "sysop", true);
        insert_user(db.db(), &user).expect("seed user");
        insert_session(
            db.db(),
            &SessionRecord {
                id: "00000000-0000-4000-8000-000000000702".to_string(),
                node_number: 1,
                user_id: Some(user.id),
                transport: "telnet".to_string(),
                remote_address: "127.0.0.1:2323".to_string(),
                remote_ip: Some("127.0.0.1".to_string()),
                remote_port: Some(2323),
                started_at: "2026-01-01T00:00:00.000000Z".to_string(),
                ended_at: None,
                disconnect_reason: None,
            },
        )
        .expect("seed open session");

        let stats = db_stats(db.db(), 0).expect("stats");
        let obj = stats.as_object().expect("stats object");

        assert_eq!(obj.get("active_sessions"), Some(&JsonValue::from(0)));
        assert_eq!(obj.get("open_sessions"), Some(&JsonValue::from(1)));
    }

    #[test]
    fn import_rejects_unsupported_format() {
        let err = require_json_format("yaml").expect_err("unsupported import format");
        assert!(err.to_string().contains("unsupported format"));
    }
}
