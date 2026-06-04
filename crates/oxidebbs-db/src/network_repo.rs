use decentdb::{Db, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkProfileRecord {
    pub id: String,
    pub key: String,
    pub name: String,
    pub adapter: String,
    pub local_zone: i64,
    pub local_net: i64,
    pub local_node: i64,
    pub local_point: i64,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkLinkRecord {
    pub id: String,
    pub key: String,
    pub network_id: String,
    pub address: String,
    pub host: String,
    pub binkp_port: i64,
    pub password: String,
    pub poll_schedule_minutes: i64,
    pub compression: String,
    pub transport_security: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkAreaRecord {
    pub id: String,
    pub network_id: String,
    pub area_tag: String,
    pub local_area_id: String,
    pub description: String,
    pub read_only: bool,
    pub subscribed: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkPacketRecord {
    pub id: String,
    pub network_id: String,
    pub direction: String,
    pub link_id: Option<String>,
    pub filename: String,
    pub sha256: String,
    pub size_bytes: i64,
    pub status: String,
    pub error_message: Option<String>,
    pub received_at: Option<String>,
    pub processed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkPacketSummaryRecord {
    pub direction: String,
    pub status: String,
    pub count: i64,
    pub total_size_bytes: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkMessageRecord {
    pub id: String,
    pub network_id: String,
    pub local_message_id: Option<String>,
    pub message_type: String,
    pub area_tag: Option<String>,
    pub origin_address: String,
    pub destination_address: Option<String>,
    pub from_name: String,
    pub to_name: Option<String>,
    pub subject: String,
    pub raw_text: Vec<u8>,
    pub display_body: String,
    pub msgid: Option<String>,
    pub replyid: Option<String>,
    pub created_at: String,
    pub imported_at: Option<String>,
    pub exported_at: Option<String>,
    pub duplicate_hash: Option<String>,
    pub packet_id: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkSeenByNode {
    pub id: String,
    pub message_id: String,
    pub network_id: String,
    pub zone: i64,
    pub net: i64,
    pub node: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkPathNode {
    pub id: String,
    pub message_id: String,
    pub network_id: String,
    pub sequence: i64,
    pub zone: i64,
    pub net: i64,
    pub node: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkDuplicateLogRecord {
    pub id: String,
    pub network_id: String,
    pub duplicate_hash: String,
    pub msgid: Option<String>,
    pub area_tag: Option<String>,
    pub origin_address: String,
    pub detected_at: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkPollLogRecord {
    pub id: String,
    pub link_id: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub direction: String,
    pub status: String,
    pub bytes_in: i64,
    pub bytes_out: i64,
    pub packets_in: i64,
    pub packets_out: i64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkSubscriptionRecord {
    pub id: String,
    pub area_id: String,
    pub link_id: String,
    pub subscribed: bool,
    pub subscribed_at: String,
    pub unsubscribed_at: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkNodelistRecord {
    pub id: String,
    pub network_id: String,
    pub zone: i64,
    pub net: i64,
    pub node: i64,
    pub point: i64,
    pub parsed_name: Option<String>,
    pub raw_entry: String,
    pub updated_at: String,
}

pub fn insert_network_profile(db: &Db, profile: &NetworkProfileRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_profiles (id, key, name, adapter, local_zone, local_net, local_node, local_point, enabled, created_at, updated_at)
         VALUES (UUID_PARSE($1), $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        &[
            Value::Text(profile.id.clone()),
            Value::Text(profile.key.clone()),
            Value::Text(profile.name.clone()),
            Value::Text(profile.adapter.clone()),
            Value::Int64(profile.local_zone),
            Value::Int64(profile.local_net),
            Value::Int64(profile.local_node),
            Value::Int64(profile.local_point),
            Value::Bool(profile.enabled),
            Value::Text(profile.created_at.clone()),
            Value::Text(profile.updated_at.clone()),
        ],
    )?;
    Ok(())
}

pub fn insert_network_link(db: &Db, link: &NetworkLinkRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_links (id, key, network_id, address, host, binkp_port, password, poll_schedule_minutes, compression, transport_security, enabled, created_at, updated_at)
         VALUES (UUID_PARSE($1), $2, UUID_PARSE($3), $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
        &[
            Value::Text(link.id.clone()),
            Value::Text(link.key.clone()),
            Value::Text(link.network_id.clone()),
            Value::Text(link.address.clone()),
            Value::Text(link.host.clone()),
            Value::Int64(link.binkp_port),
            Value::Text(link.password.clone()),
            Value::Int64(link.poll_schedule_minutes),
            Value::Text(link.compression.clone()),
            Value::Text(link.transport_security.clone()),
            Value::Bool(link.enabled),
            Value::Text(link.created_at.clone()),
            Value::Text(link.updated_at.clone()),
        ],
    )?;
    Ok(())
}

pub fn insert_network_area(db: &Db, area: &NetworkAreaRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_areas (id, network_id, area_tag, local_area_id, description, read_only, subscribed, created_at, updated_at)
         VALUES (UUID_PARSE($1), UUID_PARSE($2), $3, UUID_PARSE($4), $5, $6, $7, $8, $9)",
        &[
            Value::Text(area.id.clone()),
            Value::Text(area.network_id.clone()),
            Value::Text(area.area_tag.clone()),
            Value::Text(area.local_area_id.clone()),
            Value::Text(area.description.clone()),
            Value::Bool(area.read_only),
            Value::Bool(area.subscribed),
            Value::Text(area.created_at.clone()),
            Value::Text(area.updated_at.clone()),
        ],
    )?;
    Ok(())
}

pub fn insert_network_packet(db: &Db, packet: &NetworkPacketRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_packets (id, network_id, direction, link_id, filename, sha256, size_bytes, status, error_message, received_at, processed_at, created_at)
         VALUES (UUID_PARSE($1), UUID_PARSE($2), $3, UUID_PARSE($4), $5, $6, $7, $8, $9, $10, $11, $12)",
        &[
            Value::Text(packet.id.clone()),
            Value::Text(packet.network_id.clone()),
            Value::Text(packet.direction.clone()),
            packet
                .link_id
                .as_ref()
                .map(|id| Value::Text(id.clone()))
                .unwrap_or(Value::Null),
            Value::Text(packet.filename.clone()),
            Value::Text(packet.sha256.clone()),
            Value::Int64(packet.size_bytes),
            Value::Text(packet.status.clone()),
            packet
                .error_message
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            packet
                .received_at
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            packet
                .processed_at
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            Value::Text(packet.created_at.clone()),
        ],
    )?;
    Ok(())
}

pub fn insert_network_message(db: &Db, message: &NetworkMessageRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_messages (id, network_id, local_message_id, message_type, area_tag, origin_address, destination_address, from_name, to_name, subject, raw_text, display_body, msgid, replyid, created_at, imported_at, exported_at, duplicate_hash, packet_id, status)
         VALUES (UUID_PARSE($1), UUID_PARSE($2), UUID_PARSE($3), $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, UUID_PARSE($19), $20)",
        &[
            Value::Text(message.id.clone()),
            Value::Text(message.network_id.clone()),
            message
                .local_message_id
                .as_ref()
                .map(|id| Value::Text(id.clone()))
                .unwrap_or(Value::Null),
            Value::Text(message.message_type.clone()),
            message
                .area_tag
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            Value::Text(message.origin_address.clone()),
            message
                .destination_address
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            Value::Text(message.from_name.clone()),
            message
                .to_name
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            Value::Text(message.subject.clone()),
            Value::Blob(message.raw_text.clone()),
            Value::Text(message.display_body.clone()),
            message
                .msgid
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            message
                .replyid
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            Value::Text(message.created_at.clone()),
            message
                .imported_at
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            message
                .exported_at
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            message
                .duplicate_hash
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            message
                .packet_id
                .as_ref()
                .map(|id| Value::Text(id.clone()))
                .unwrap_or(Value::Null),
            Value::Text(message.status.clone()),
        ],
    )?;
    Ok(())
}

pub fn insert_network_seen_by(db: &Db, seen_by: &NetworkSeenByNode) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_seen_by (id, message_id, network_id, zone, net, node)
         VALUES (UUID_PARSE($1), UUID_PARSE($2), UUID_PARSE($3), $4, $5, $6)",
        &[
            Value::Text(seen_by.id.clone()),
            Value::Text(seen_by.message_id.clone()),
            Value::Text(seen_by.network_id.clone()),
            Value::Int64(seen_by.zone),
            Value::Int64(seen_by.net),
            Value::Int64(seen_by.node),
        ],
    )?;
    Ok(())
}

pub fn insert_network_seen_by_node(db: &Db, seen_by: &NetworkSeenByNode) -> decentdb::Result<()> {
    insert_network_seen_by(db, seen_by)
}

pub fn insert_network_path_node(db: &Db, node: &NetworkPathNode) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_path (id, message_id, network_id, sequence, zone, net, node)
         VALUES (UUID_PARSE($1), UUID_PARSE($2), UUID_PARSE($3), $4, $5, $6, $7)",
        &[
            Value::Text(node.id.clone()),
            Value::Text(node.message_id.clone()),
            Value::Text(node.network_id.clone()),
            Value::Int64(node.sequence),
            Value::Int64(node.zone),
            Value::Int64(node.net),
            Value::Int64(node.node),
        ],
    )?;
    Ok(())
}

pub fn insert_network_path(db: &Db, path: &[NetworkPathNode]) -> decentdb::Result<()> {
    db.begin_transaction()?;
    for node in path {
        insert_network_path_node(db, node)?;
    }
    db.commit_transaction()?;
    Ok(())
}

pub fn insert_network_duplicate_log(
    db: &Db,
    log: &NetworkDuplicateLogRecord,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_duplicate_log (id, network_id, duplicate_hash, msgid, area_tag, origin_address, detected_at, action)
         VALUES (UUID_PARSE($1), UUID_PARSE($2), $3, $4, $5, $6, $7, $8)",
        &[
            Value::Text(log.id.clone()),
            Value::Text(log.network_id.clone()),
            Value::Text(log.duplicate_hash.clone()),
            log.msgid
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            log.area_tag
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            Value::Text(log.origin_address.clone()),
            Value::Text(log.detected_at.clone()),
            Value::Text(log.action.clone()),
        ],
    )?;
    Ok(())
}

#[allow(dead_code)]
pub fn insert_network_poll_log(db: &Db, log: &NetworkPollLogRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_poll_log (id, link_id, started_at, ended_at, direction, status, bytes_in, bytes_out, packets_in, packets_out, error_message)
         VALUES (UUID_PARSE($1), UUID_PARSE($2), $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        &[
            Value::Text(log.id.clone()),
            Value::Text(log.link_id.clone()),
            Value::Text(log.started_at.clone()),
            log.ended_at
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            Value::Text(log.direction.clone()),
            Value::Text(log.status.clone()),
            Value::Int64(log.bytes_in),
            Value::Int64(log.bytes_out),
            Value::Int64(log.packets_in),
            Value::Int64(log.packets_out),
            log.error_message
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
        ],
    )?;
    Ok(())
}

pub fn insert_network_subscription(
    db: &Db,
    subscription: &NetworkSubscriptionRecord,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_area_subscriptions (id, area_id, link_id, subscribed, subscribed_at, unsubscribed_at, source)
         VALUES (UUID_PARSE($1), UUID_PARSE($2), UUID_PARSE($3), $4, $5, $6, $7)",
        &[
            Value::Text(subscription.id.clone()),
            Value::Text(subscription.area_id.clone()),
            Value::Text(subscription.link_id.clone()),
            Value::Bool(subscription.subscribed),
            Value::Text(subscription.subscribed_at.clone()),
            subscription
                .unsubscribed_at
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            Value::Text(subscription.source.clone()),
        ],
    )?;
    Ok(())
}

pub fn insert_network_nodelist_entry(
    db: &Db,
    entry: &NetworkNodelistRecord,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_nodelist (id, network_id, zone, net, node, point, parsed_name, raw_entry, updated_at)
         VALUES (UUID_PARSE($1), UUID_PARSE($2), $3, $4, $5, $6, $7, $8, $9)",
        &[
            Value::Text(entry.id.clone()),
            Value::Text(entry.network_id.clone()),
            Value::Int64(entry.zone),
            Value::Int64(entry.net),
            Value::Int64(entry.node),
            Value::Int64(entry.point),
            entry
                .parsed_name
                .as_ref()
                .map(|value| Value::Text(value.clone()))
                .unwrap_or(Value::Null),
            Value::Text(entry.raw_entry.clone()),
            Value::Text(entry.updated_at.clone()),
        ],
    )?;
    Ok(())
}

pub fn replace_network_nodelist_entries(
    db: &Db,
    network_id: &str,
    entries: &[NetworkNodelistRecord],
) -> decentdb::Result<()> {
    db.begin_transaction()?;
    let result = (|| {
        db.execute_with_params(
            "DELETE FROM network_nodelist WHERE network_id = UUID_PARSE($1)",
            &[Value::Text(network_id.to_string())],
        )?;
        for entry in entries {
            insert_network_nodelist_entry(db, entry)?;
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

pub fn list_network_profiles(db: &Db) -> decentdb::Result<Vec<NetworkProfileRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), key, name, adapter, local_zone, local_net, local_node, local_point, enabled, CAST(created_at AS TEXT), CAST(updated_at AS TEXT)
         FROM network_profiles ORDER BY key",
    )?;
    Ok(result.rows().iter().map(network_profile_from_row).collect())
}

pub fn list_network_links(db: &Db) -> decentdb::Result<Vec<NetworkLinkRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), key, UUID_TO_STRING(network_id), address, host, binkp_port, password, poll_schedule_minutes, compression, transport_security, enabled, CAST(created_at AS TEXT), CAST(updated_at AS TEXT)
         FROM network_links ORDER BY key",
    )?;
    Ok(result.rows().iter().map(network_link_from_row).collect())
}

pub fn list_network_areas(db: &Db) -> decentdb::Result<Vec<NetworkAreaRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), area_tag, UUID_TO_STRING(local_area_id), description, read_only, subscribed, CAST(created_at AS TEXT), CAST(updated_at AS TEXT)
         FROM network_areas ORDER BY area_tag",
    )?;
    Ok(result.rows().iter().map(network_area_from_row).collect())
}

pub fn list_network_packets(db: &Db) -> decentdb::Result<Vec<NetworkPacketRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), direction, UUID_TO_STRING(link_id), filename, sha256, size_bytes, status, error_message, CAST(received_at AS TEXT), CAST(processed_at AS TEXT), CAST(created_at AS TEXT)
         FROM network_packets ORDER BY created_at DESC",
    )?;
    Ok(result.rows().iter().map(network_packet_from_row).collect())
}

pub fn find_network_packet_by_id(
    db: &Db,
    packet_id: &str,
) -> decentdb::Result<Option<NetworkPacketRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), direction, UUID_TO_STRING(link_id), filename, sha256, size_bytes, status, error_message, CAST(received_at AS TEXT), CAST(processed_at AS TEXT), CAST(created_at AS TEXT)
         FROM network_packets WHERE id = UUID_PARSE($1)",
        &[Value::Text(packet_id.to_string())],
    )?;
    Ok(result.rows().first().map(network_packet_from_row))
}

pub fn summarize_network_packets(
    db: &Db,
    network_id: Option<&str>,
) -> decentdb::Result<Vec<NetworkPacketSummaryRecord>> {
    let sql = "SELECT direction, status, COUNT(*), COALESCE(SUM(size_bytes), 0)
         FROM network_packets
         GROUP BY direction, status
         ORDER BY direction, status";
    let result = match network_id {
        Some(network_id) => db.execute_with_params(
            "SELECT direction, status, COUNT(*), COALESCE(SUM(size_bytes), 0)
             FROM network_packets
             WHERE network_id = UUID_PARSE($1)
             GROUP BY direction, status
             ORDER BY direction, status",
            &[Value::Text(network_id.to_string())],
        )?,
        None => db.execute(sql)?,
    };
    Ok(result
        .rows()
        .iter()
        .map(network_packet_summary_from_row)
        .collect())
}

pub fn list_network_messages(db: &Db) -> decentdb::Result<Vec<NetworkMessageRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), UUID_TO_STRING(local_message_id), message_type, area_tag, origin_address, destination_address, from_name, to_name, subject, raw_text, display_body, msgid, replyid, CAST(created_at AS TEXT), CAST(imported_at AS TEXT), CAST(exported_at AS TEXT), duplicate_hash, UUID_TO_STRING(packet_id), status
         FROM network_messages ORDER BY created_at DESC",
    )?;
    Ok(result.rows().iter().map(network_message_from_row).collect())
}

pub fn list_network_duplicates(db: &Db) -> decentdb::Result<Vec<NetworkDuplicateLogRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), duplicate_hash, msgid, area_tag, origin_address, CAST(detected_at AS TEXT), action
         FROM network_duplicate_log ORDER BY detected_at DESC",
    )?;
    Ok(result
        .rows()
        .iter()
        .map(network_duplicate_log_from_row)
        .collect())
}

pub fn list_network_poll_logs(db: &Db) -> decentdb::Result<Vec<NetworkPollLogRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(link_id), CAST(started_at AS TEXT), CAST(ended_at AS TEXT), direction, status, bytes_in, bytes_out, packets_in, packets_out, error_message
         FROM network_poll_log ORDER BY started_at DESC",
    )?;
    Ok(result
        .rows()
        .iter()
        .map(network_poll_log_from_row)
        .collect())
}

pub fn list_network_seen_by(db: &Db) -> decentdb::Result<Vec<NetworkSeenByNode>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(message_id), UUID_TO_STRING(network_id), zone, net, node
         FROM network_seen_by ORDER BY id",
    )?;
    Ok(result.rows().iter().map(network_seen_by_from_row).collect())
}

pub fn list_network_path(db: &Db) -> decentdb::Result<Vec<NetworkPathNode>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(message_id), UUID_TO_STRING(network_id), sequence, zone, net, node
         FROM network_path ORDER BY message_id, sequence",
    )?;
    Ok(result.rows().iter().map(network_path_from_row).collect())
}

pub fn list_network_subscriptions(db: &Db) -> decentdb::Result<Vec<NetworkSubscriptionRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(area_id), UUID_TO_STRING(link_id), subscribed, CAST(subscribed_at AS TEXT), CAST(unsubscribed_at AS TEXT), source
         FROM network_area_subscriptions ORDER BY area_id, link_id",
    )?;
    Ok(result
        .rows()
        .iter()
        .map(network_subscription_from_row)
        .collect())
}

pub fn list_network_nodelist_entries(db: &Db) -> decentdb::Result<Vec<NetworkNodelistRecord>> {
    let result = db.execute(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), zone, net, node, point, parsed_name, raw_entry, CAST(updated_at AS TEXT)
         FROM network_nodelist ORDER BY network_id, zone, net, node",
    )?;
    Ok(result
        .rows()
        .iter()
        .map(network_nodelist_from_row)
        .collect())
}

pub fn find_network_nodelist_entry(
    db: &Db,
    network_id: &str,
    zone: i64,
    net: i64,
    node: i64,
    point: i64,
) -> decentdb::Result<Option<NetworkNodelistRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), zone, net, node, point, parsed_name, raw_entry, CAST(updated_at AS TEXT)
         FROM network_nodelist
         WHERE network_id = UUID_PARSE($1) AND zone = $2 AND net = $3 AND node = $4 AND point = $5",
        &[
            Value::Text(network_id.to_string()),
            Value::Int64(zone),
            Value::Int64(net),
            Value::Int64(node),
            Value::Int64(point),
        ],
    )?;
    Ok(result.rows().first().map(network_nodelist_from_row))
}

pub fn find_network_profile_by_key(
    db: &Db,
    key: &str,
) -> decentdb::Result<Option<NetworkProfileRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), key, name, adapter, local_zone, local_net, local_node, local_point, enabled, CAST(created_at AS TEXT), CAST(updated_at AS TEXT)
         FROM network_profiles WHERE key = $1",
        &[Value::Text(key.to_string())],
    )?;
    Ok(result.rows().first().map(network_profile_from_row))
}

pub fn find_network_profile_by_id(
    db: &Db,
    id: &str,
) -> decentdb::Result<Option<NetworkProfileRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), key, name, adapter, local_zone, local_net, local_node, local_point, enabled, CAST(created_at AS TEXT), CAST(updated_at AS TEXT)
         FROM network_profiles WHERE id = UUID_PARSE($1)",
        &[Value::Text(id.to_string())],
    )?;
    Ok(result.rows().first().map(network_profile_from_row))
}

pub fn find_network_link_by_key(db: &Db, key: &str) -> decentdb::Result<Option<NetworkLinkRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), key, UUID_TO_STRING(network_id), address, host, binkp_port, password, poll_schedule_minutes, compression, transport_security, enabled, CAST(created_at AS TEXT), CAST(updated_at AS TEXT)
         FROM network_links WHERE key = $1",
        &[Value::Text(key.to_string())],
    )?;
    Ok(result.rows().first().map(network_link_from_row))
}

pub fn find_network_area_by_tag_and_profile(
    db: &Db,
    network_id: &str,
    area_tag: &str,
) -> decentdb::Result<Option<NetworkAreaRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), area_tag, UUID_TO_STRING(local_area_id), description, read_only, subscribed, CAST(created_at AS TEXT), CAST(updated_at AS TEXT)
         FROM network_areas WHERE network_id = UUID_PARSE($1) AND area_tag = $2",
        &[Value::Text(network_id.to_string()), Value::Text(area_tag.to_string())],
    )?;
    Ok(result.rows().first().map(network_area_from_row))
}

pub fn set_network_profile_enabled(
    db: &Db,
    network_profile_id: &str,
    enabled: bool,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE network_profiles SET enabled = $1 WHERE id = UUID_PARSE($2)",
        &[
            Value::Bool(enabled),
            Value::Text(network_profile_id.to_string()),
        ],
    )?;
    Ok(())
}

pub fn set_network_area_subscribed(
    db: &Db,
    area_id: &str,
    subscribed: bool,
) -> decentdb::Result<bool> {
    let result = db.execute_with_params(
        "UPDATE network_areas SET subscribed = $1, updated_at = CURRENT_TIMESTAMP WHERE id = UUID_PARSE($2)",
        &[Value::Bool(subscribed), Value::Text(area_id.to_string())],
    )?;
    Ok(result.affected_rows() > 0)
}

pub fn set_network_subscription_status(
    db: &Db,
    area_id: &str,
    link_id: &str,
    subscribed: bool,
    timestamp: &str,
    source: &str,
) -> decentdb::Result<bool> {
    let sql = if subscribed {
        "UPDATE network_area_subscriptions
         SET subscribed = TRUE, subscribed_at = $1, unsubscribed_at = NULL, source = $2
         WHERE area_id = UUID_PARSE($3) AND link_id = UUID_PARSE($4)"
    } else {
        "UPDATE network_area_subscriptions
         SET subscribed = FALSE, unsubscribed_at = $1, source = $2
         WHERE area_id = UUID_PARSE($3) AND link_id = UUID_PARSE($4)"
    };
    let result = db.execute_with_params(
        sql,
        &[
            Value::Text(timestamp.to_string()),
            Value::Text(source.to_string()),
            Value::Text(area_id.to_string()),
            Value::Text(link_id.to_string()),
        ],
    )?;
    Ok(result.affected_rows() > 0)
}

pub fn finish_network_packet(
    db: &Db,
    packet_id: &str,
    status: &str,
    error_message: Option<&str>,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE network_packets SET status = $1, processed_at = CURRENT_TIMESTAMP, error_message = $2 WHERE id = UUID_PARSE($3)",
        &[
            Value::Text(status.to_string()),
            error_message
                .map(|value| Value::Text(value.to_string()))
                .unwrap_or(Value::Null),
            Value::Text(packet_id.to_string()),
        ],
    )?;
    Ok(())
}

pub fn requeue_network_packet(db: &Db, packet_id: &str) -> decentdb::Result<bool> {
    let result = db.execute_with_params(
        "UPDATE network_packets
         SET status = 'pending', error_message = NULL, processed_at = NULL
         WHERE id = UUID_PARSE($1)",
        &[Value::Text(packet_id.to_string())],
    )?;
    Ok(result.affected_rows() > 0)
}

pub fn mark_network_packet_quarantined(
    db: &Db,
    packet_id: &str,
    reason: &str,
) -> decentdb::Result<bool> {
    let result = db.execute_with_params(
        "UPDATE network_packets
         SET status = 'quarantined', error_message = $1, processed_at = CURRENT_TIMESTAMP
         WHERE id = UUID_PARSE($2)",
        &[
            Value::Text(reason.to_string()),
            Value::Text(packet_id.to_string()),
        ],
    )?;
    Ok(result.affected_rows() > 0)
}

pub fn finish_network_poll(
    db: &Db,
    poll_id: &str,
    status: &str,
    error_message: Option<&str>,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "UPDATE network_poll_log SET status = $1, ended_at = CURRENT_TIMESTAMP, error_message = $2 WHERE id = UUID_PARSE($3)",
        &[
            Value::Text(status.to_string()),
            error_message
                .map(|value| Value::Text(value.to_string()))
                .unwrap_or(Value::Null),
            Value::Text(poll_id.to_string()),
        ],
    )?;
    Ok(())
}

fn network_profile_from_row(row: &decentdb::QueryRow) -> NetworkProfileRecord {
    let values = row.values();
    NetworkProfileRecord {
        id: text_value(&values[0]),
        key: text_value(&values[1]),
        name: text_value(&values[2]),
        adapter: text_value(&values[3]),
        local_zone: int_value(&values[4]),
        local_net: int_value(&values[5]),
        local_node: int_value(&values[6]),
        local_point: int_value(&values[7]),
        enabled: bool_value(&values[8]),
        created_at: text_value(&values[9]),
        updated_at: text_value(&values[10]),
    }
}

fn network_link_from_row(row: &decentdb::QueryRow) -> NetworkLinkRecord {
    let values = row.values();
    NetworkLinkRecord {
        id: text_value(&values[0]),
        key: text_value(&values[1]),
        network_id: text_value(&values[2]),
        address: text_value(&values[3]),
        host: text_value(&values[4]),
        binkp_port: int_value(&values[5]),
        password: text_value(&values[6]),
        poll_schedule_minutes: int_value(&values[7]),
        compression: text_value(&values[8]),
        transport_security: text_value(&values[9]),
        enabled: bool_value(&values[10]),
        created_at: text_value(&values[11]),
        updated_at: text_value(&values[12]),
    }
}

fn network_area_from_row(row: &decentdb::QueryRow) -> NetworkAreaRecord {
    let values = row.values();
    NetworkAreaRecord {
        id: text_value(&values[0]),
        network_id: text_value(&values[1]),
        area_tag: text_value(&values[2]),
        local_area_id: text_value(&values[3]),
        description: text_value(&values[4]),
        read_only: bool_value(&values[5]),
        subscribed: bool_value(&values[6]),
        created_at: text_value(&values[7]),
        updated_at: text_value(&values[8]),
    }
}

fn network_packet_from_row(row: &decentdb::QueryRow) -> NetworkPacketRecord {
    let values = row.values();
    NetworkPacketRecord {
        id: text_value(&values[0]),
        network_id: text_value(&values[1]),
        direction: text_value(&values[2]),
        link_id: opt_text_value(&values[3]),
        filename: text_value(&values[4]),
        sha256: text_value(&values[5]),
        size_bytes: int_value(&values[6]),
        status: text_value(&values[7]),
        error_message: opt_text_value(&values[8]),
        received_at: opt_text_value(&values[9]),
        processed_at: opt_text_value(&values[10]),
        created_at: text_value(&values[11]),
    }
}

fn network_packet_summary_from_row(row: &decentdb::QueryRow) -> NetworkPacketSummaryRecord {
    let values = row.values();
    NetworkPacketSummaryRecord {
        direction: text_value(&values[0]),
        status: text_value(&values[1]),
        count: int_value(&values[2]),
        total_size_bytes: int_value(&values[3]),
    }
}

fn network_message_from_row(row: &decentdb::QueryRow) -> NetworkMessageRecord {
    let values = row.values();
    NetworkMessageRecord {
        id: text_value(&values[0]),
        network_id: text_value(&values[1]),
        local_message_id: opt_text_value(&values[2]),
        message_type: text_value(&values[3]),
        area_tag: opt_text_value(&values[4]),
        origin_address: text_value(&values[5]),
        destination_address: opt_text_value(&values[6]),
        from_name: text_value(&values[7]),
        to_name: opt_text_value(&values[8]),
        subject: text_value(&values[9]),
        raw_text: blob_value(&values[10]),
        display_body: text_value(&values[11]),
        msgid: opt_text_value(&values[12]),
        replyid: opt_text_value(&values[13]),
        created_at: text_value(&values[14]),
        imported_at: opt_text_value(&values[15]),
        exported_at: opt_text_value(&values[16]),
        duplicate_hash: opt_text_value(&values[17]),
        packet_id: opt_text_value(&values[18]),
        status: text_value(&values[19]),
    }
}

fn network_seen_by_from_row(row: &decentdb::QueryRow) -> NetworkSeenByNode {
    let values = row.values();
    NetworkSeenByNode {
        id: text_value(&values[0]),
        message_id: text_value(&values[1]),
        network_id: text_value(&values[2]),
        zone: int_value(&values[3]),
        net: int_value(&values[4]),
        node: int_value(&values[5]),
    }
}

fn network_path_from_row(row: &decentdb::QueryRow) -> NetworkPathNode {
    let values = row.values();
    NetworkPathNode {
        id: text_value(&values[0]),
        message_id: text_value(&values[1]),
        network_id: text_value(&values[2]),
        sequence: int_value(&values[3]),
        zone: int_value(&values[4]),
        net: int_value(&values[5]),
        node: int_value(&values[6]),
    }
}

fn network_duplicate_log_from_row(row: &decentdb::QueryRow) -> NetworkDuplicateLogRecord {
    let values = row.values();
    NetworkDuplicateLogRecord {
        id: text_value(&values[0]),
        network_id: text_value(&values[1]),
        duplicate_hash: text_value(&values[2]),
        msgid: opt_text_value(&values[3]),
        area_tag: opt_text_value(&values[4]),
        origin_address: text_value(&values[5]),
        detected_at: text_value(&values[6]),
        action: text_value(&values[7]),
    }
}

fn network_poll_log_from_row(row: &decentdb::QueryRow) -> NetworkPollLogRecord {
    let values = row.values();
    NetworkPollLogRecord {
        id: text_value(&values[0]),
        link_id: text_value(&values[1]),
        started_at: text_value(&values[2]),
        ended_at: opt_text_value(&values[3]),
        direction: text_value(&values[4]),
        status: text_value(&values[5]),
        bytes_in: int_value(&values[6]),
        bytes_out: int_value(&values[7]),
        packets_in: int_value(&values[8]),
        packets_out: int_value(&values[9]),
        error_message: opt_text_value(&values[10]),
    }
}

fn network_subscription_from_row(row: &decentdb::QueryRow) -> NetworkSubscriptionRecord {
    let values = row.values();
    NetworkSubscriptionRecord {
        id: text_value(&values[0]),
        area_id: text_value(&values[1]),
        link_id: text_value(&values[2]),
        subscribed: bool_value(&values[3]),
        subscribed_at: text_value(&values[4]),
        unsubscribed_at: opt_text_value(&values[5]),
        source: text_value(&values[6]),
    }
}

fn network_nodelist_from_row(row: &decentdb::QueryRow) -> NetworkNodelistRecord {
    let values = row.values();
    NetworkNodelistRecord {
        id: text_value(&values[0]),
        network_id: text_value(&values[1]),
        zone: int_value(&values[2]),
        net: int_value(&values[3]),
        node: int_value(&values[4]),
        point: int_value(&values[5]),
        parsed_name: opt_text_value(&values[6]),
        raw_entry: text_value(&values[7]),
        updated_at: text_value(&values[8]),
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

fn bool_value(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        _ => false,
    }
}

fn blob_value(value: &Value) -> Vec<u8> {
    match value {
        Value::Blob(blob) => blob.clone(),
        _ => Vec::new(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NetworkRescanQueueRecord {
    pub id: String,
    pub network_id: String,
    pub link_id: String,
    pub area_tag: String,
    pub status: String,
    pub requested_at: String,
    pub processed_at: Option<String>,
}

pub fn insert_network_rescan_queue(
    db: &Db,
    record: &NetworkRescanQueueRecord,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_rescan_queue (id, network_id, link_id, area_tag, status, requested_at, processed_at)
         VALUES (UUID_PARSE($1), UUID_PARSE($2), UUID_PARSE($3), $4, $5, $6, $7)",
        &[
            Value::Text(record.id.clone()),
            Value::Text(record.network_id.clone()),
            Value::Text(record.link_id.clone()),
            Value::Text(record.area_tag.clone()),
            Value::Text(record.status.clone()),
            Value::Text(record.requested_at.clone()),
            record
                .processed_at
                .as_ref()
                .map(|s| Value::Text(s.clone()))
                .unwrap_or(Value::Null),
        ],
    )?;
    Ok(())
}

/// List all rescan queue entries, optionally filtered by network and status.
pub fn list_network_rescan_queue(
    db: &Db,
    network_id: Option<&str>,
    status: Option<&str>,
) -> decentdb::Result<Vec<NetworkRescanQueueRecord>> {
    let (sql, params) = match (network_id, status) {
        (Some(nid), Some(st)) => (
            "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), UUID_TO_STRING(link_id),
                    area_tag, status, CAST(requested_at AS TEXT), CAST(processed_at AS TEXT)
             FROM network_rescan_queue
             WHERE network_id = UUID_PARSE($1) AND status = $2
             ORDER BY requested_at ASC",
            vec![Value::Text(nid.to_string()), Value::Text(st.to_string())],
        ),
        (Some(nid), None) => (
            "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), UUID_TO_STRING(link_id),
                    area_tag, status, CAST(requested_at AS TEXT), CAST(processed_at AS TEXT)
             FROM network_rescan_queue
             WHERE network_id = UUID_PARSE($1)
             ORDER BY requested_at ASC",
            vec![Value::Text(nid.to_string())],
        ),
        (None, Some(st)) => (
            "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), UUID_TO_STRING(link_id),
                    area_tag, status, CAST(requested_at AS TEXT), CAST(processed_at AS TEXT)
             FROM network_rescan_queue
             WHERE status = $1
             ORDER BY requested_at ASC",
            vec![Value::Text(st.to_string())],
        ),
        (None, None) => (
            "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), UUID_TO_STRING(link_id),
                    area_tag, status, CAST(requested_at AS TEXT), CAST(processed_at AS TEXT)
             FROM network_rescan_queue
             ORDER BY requested_at ASC",
            vec![],
        ),
    };

    let result = db.execute_with_params(sql, &params)?;
    Ok(result.rows().iter().map(rescan_queue_from_row).collect())
}

fn rescan_queue_from_row(row: &decentdb::QueryRow) -> NetworkRescanQueueRecord {
    let values = row.values();
    NetworkRescanQueueRecord {
        id: text_value(&values[0]),
        network_id: text_value(&values[1]),
        link_id: text_value(&values[2]),
        area_tag: text_value(&values[3]),
        status: text_value(&values[4]),
        requested_at: text_value(&values[5]),
        processed_at: opt_text_value(&values[6]),
    }
}

/// Update the status of a rescan queue entry.
pub fn update_network_rescan_status(
    db: &Db,
    rescan_id: &str,
    status: &str,
    processed_at: Option<&str>,
) -> decentdb::Result<bool> {
    let result = db.execute_with_params(
        "UPDATE network_rescan_queue
         SET status = $1, processed_at = $2
         WHERE id = UUID_PARSE($3)",
        &[
            Value::Text(status.to_string()),
            processed_at
                .map(|s| Value::Text(s.to_string()))
                .unwrap_or(Value::Null),
            Value::Text(rescan_id.to_string()),
        ],
    )?;
    Ok(result.affected_rows() > 0)
}

/// Find a rescan queue entry by ID.
pub fn find_network_rescan_by_id(
    db: &Db,
    rescan_id: &str,
) -> decentdb::Result<Option<NetworkRescanQueueRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), UUID_TO_STRING(link_id),
                area_tag, status, CAST(requested_at AS TEXT), CAST(processed_at AS TEXT)
         FROM network_rescan_queue
         WHERE id = UUID_PARSE($1)",
        &[Value::Text(rescan_id.to_string())],
    )?;
    Ok(result.rows().first().map(rescan_queue_from_row))
}

/// Count network packets created before the cutoff timestamp.
///
/// Only counts packets with terminal status (processed, failed) to avoid
/// affecting active or quarantined packets that need manual review.
pub fn count_network_packets_before(db: &Db, cutoff_timestamp: &str) -> decentdb::Result<i64> {
    let result = db.execute_with_params(
        "SELECT COUNT(*) FROM network_packets
         WHERE created_at < CAST($1 AS TIMESTAMPTZ)
         AND status IN ('processed', 'failed')",
        &[Value::Text(cutoff_timestamp.to_string())],
    )?;
    Ok(result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .and_then(|value| match value {
            Value::Int64(count) => Some(*count),
            _ => None,
        })
        .unwrap_or(0))
}

/// Delete network packets and associated records older than the cutoff timestamp.
///
/// Only deletes packets with terminal status (processed, failed) to avoid
/// affecting active or quarantined packets that need manual review.
/// Returns the number of packets deleted.
pub fn delete_network_packets_older_than(db: &Db, cutoff_timestamp: &str) -> decentdb::Result<i64> {
    let before = count_network_packets_before(db, cutoff_timestamp)?;

    // Delete associated records first (foreign key constraints)
    db.execute_with_params(
        "DELETE FROM network_messages
         WHERE packet_id IN (
             SELECT id FROM network_packets
             WHERE created_at < CAST($1 AS TIMESTAMPTZ)
             AND status IN ('processed', 'failed')
         )",
        &[Value::Text(cutoff_timestamp.to_string())],
    )?;

    db.execute_with_params(
        "DELETE FROM network_seen_by
         WHERE message_id IN (
             SELECT id FROM network_messages
             WHERE packet_id IN (
                 SELECT id FROM network_packets
                 WHERE created_at < CAST($1 AS TIMESTAMPTZ)
                 AND status IN ('processed', 'failed')
             )
         )",
        &[Value::Text(cutoff_timestamp.to_string())],
    )?;

    db.execute_with_params(
        "DELETE FROM network_path
         WHERE message_id IN (
             SELECT id FROM network_messages
             WHERE packet_id IN (
                 SELECT id FROM network_packets
                 WHERE created_at < CAST($1 AS TIMESTAMPTZ)
                 AND status IN ('processed', 'failed')
             )
         )",
        &[Value::Text(cutoff_timestamp.to_string())],
    )?;

    // Delete the packets themselves
    db.execute_with_params(
        "DELETE FROM network_packets
         WHERE created_at < CAST($1 AS TIMESTAMPTZ)
         AND status IN ('processed', 'failed')",
        &[Value::Text(cutoff_timestamp.to_string())],
    )?;

    let after = count_network_packets_before(db, cutoff_timestamp)?;
    Ok(before.saturating_sub(after))
}

/// List network packets eligible for retention cleanup.
///
/// Returns packets with terminal status (processed, failed) created before
/// the cutoff timestamp, ordered by creation date (oldest first).
pub fn list_network_packets_for_retention(
    db: &Db,
    cutoff_timestamp: &str,
    limit: i64,
) -> decentdb::Result<Vec<NetworkPacketRecord>> {
    let result = db.execute_with_params(
        "SELECT UUID_TO_STRING(id), UUID_TO_STRING(network_id), direction, UUID_TO_STRING(link_id),
                filename, sha256, size_bytes, status, error_message,
                CAST(received_at AS TEXT), CAST(processed_at AS TEXT), CAST(created_at AS TEXT)
         FROM network_packets
         WHERE created_at < CAST($1 AS TIMESTAMPTZ)
         AND status IN ('processed', 'failed')
         ORDER BY created_at ASC
         LIMIT $2",
        &[
            Value::Text(cutoff_timestamp.to_string()),
            Value::Int64(limit),
        ],
    )?;
    Ok(result.rows().iter().map(network_packet_from_row).collect())
}

/// Cumulative FTN operations statistics for a network profile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NetworkOperationsStats {
    /// Number of inbound packets successfully processed (status = 'processed')
    pub packets_tossed: i64,
    /// Number of inbound packets quarantined (status = 'quarantined')
    pub packets_quarantined: i64,
    /// Number of outbound packets created (direction = 'outbound')
    pub packets_scanned: i64,
    /// Number of messages imported (status = 'imported')
    pub messages_imported: i64,
    /// Number of messages exported (status = 'exported')
    pub messages_exported: i64,
    /// Number of duplicate messages detected (from network_duplicate_log)
    pub duplicates_detected: i64,
    /// Number of successful polls (status = 'success')
    pub polls_succeeded: i64,
    /// Number of failed polls (status = 'failed' or 'timeout')
    pub polls_failed: i64,
    /// Total bytes received across all polls
    pub bytes_received: i64,
    /// Total bytes sent across all polls
    pub bytes_sent: i64,
}

/// Compute cumulative FTN operations statistics for a network profile.
///
/// Aggregates statistics from network_packets, network_messages,
/// network_duplicate_log, and network_poll_log tables.
pub fn get_network_operations_stats(
    db: &Db,
    network_id: &str,
) -> decentdb::Result<NetworkOperationsStats> {
    let mut stats = NetworkOperationsStats::default();

    // Packet stats
    let packet_result = db.execute_with_params(
        "SELECT status, COUNT(*) as count
         FROM network_packets
         WHERE network_id = UUID_PARSE($1)
         GROUP BY status",
        &[Value::Text(network_id.to_string())],
    )?;
    for row in packet_result.rows() {
        if let (Some(Value::Text(status)), Some(Value::Int64(count))) =
            (row.values().first(), row.values().get(1))
        {
            match status.as_str() {
                "processed" => stats.packets_tossed = *count,
                "quarantined" => stats.packets_quarantined = *count,
                _ => {}
            }
        }
    }

    // Outbound packets count
    let outbound_result = db.execute_with_params(
        "SELECT COUNT(*) FROM network_packets
         WHERE network_id = UUID_PARSE($1) AND direction = 'outbound'",
        &[Value::Text(network_id.to_string())],
    )?;
    if let Some(row) = outbound_result.rows().first()
        && let Some(Value::Int64(count)) = row.values().first()
    {
        stats.packets_scanned = *count;
    }

    // Message stats
    let message_result = db.execute_with_params(
        "SELECT status, COUNT(*) as count
         FROM network_messages
         WHERE network_id = UUID_PARSE($1)
         GROUP BY status",
        &[Value::Text(network_id.to_string())],
    )?;
    for row in message_result.rows() {
        if let (Some(Value::Text(status)), Some(Value::Int64(count))) =
            (row.values().first(), row.values().get(1))
        {
            match status.as_str() {
                "imported" => stats.messages_imported = *count,
                "exported" => stats.messages_exported = *count,
                _ => {}
            }
        }
    }

    // Duplicate stats
    let dup_result = db.execute_with_params(
        "SELECT COUNT(*) FROM network_duplicate_log
         WHERE network_id = UUID_PARSE($1)",
        &[Value::Text(network_id.to_string())],
    )?;
    if let Some(row) = dup_result.rows().first()
        && let Some(Value::Int64(count)) = row.values().first()
    {
        stats.duplicates_detected = *count;
    }

    // Poll stats
    let poll_result = db.execute_with_params(
        "SELECT p.status, COUNT(*) as count,
                COALESCE(SUM(p.bytes_in), 0) as total_bytes_in,
                COALESCE(SUM(p.bytes_out), 0) as total_bytes_out
         FROM network_poll_log p
         INNER JOIN network_links l ON p.link_id = l.id
         WHERE l.network_id = UUID_PARSE($1)
         GROUP BY p.status",
        &[Value::Text(network_id.to_string())],
    )?;
    for row in poll_result.rows() {
        if let Some(Value::Text(status)) = row.values().first() {
            if let Some(Value::Int64(count)) = row.values().get(1) {
                match status.as_str() {
                    "success" => stats.polls_succeeded = *count,
                    "failed" | "timeout" => stats.polls_failed += count,
                    _ => {}
                }
            }
            if let Some(Value::Int64(bytes_in)) = row.values().get(2) {
                stats.bytes_received += bytes_in;
            }
            if let Some(Value::Int64(bytes_out)) = row.values().get(3) {
                stats.bytes_sent += bytes_out;
            }
        }
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use decentdb::DbConfig;

    const PROFILE_ID: &str = "00000000-0000-4000-8000-100000000001";
    const PROFILE_KEY: &str = "legacy-ftn";
    const LINK_ID: &str = "00000000-0000-4000-8000-100000000002";
    const AREA_ID: &str = "00000000-0000-4000-8000-100000000003";
    const LOCAL_AREA_ID: &str = "00000000-0000-4000-8000-100000000004";
    const MESSAGE_ID: &str = "00000000-0000-4000-8000-100000000005";
    const PACKET_ID: &str = "00000000-0000-4000-8000-100000000006";
    const SECOND_PACKET_ID: &str = "00000000-0000-4000-8000-100000000007";
    const DUP_LOG_ID: &str = "00000000-0000-4000-8000-100000000008";
    const POLL_ID: &str = "00000000-0000-4000-8000-100000000009";
    const SUBSCRIPTION_ID: &str = "00000000-0000-4000-8000-100000000010";
    const NODELIST_ID: &str = "00000000-0000-4000-8000-100000000011";

    fn test_db() -> Db {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open db");
        schema::init_schema(&db).expect("init schema");
        db
    }

    fn profile() -> NetworkProfileRecord {
        NetworkProfileRecord {
            id: PROFILE_ID.to_string(),
            key: PROFILE_KEY.to_string(),
            name: "Legacy FTN".to_string(),
            adapter: "legacy-ftn".to_string(),
            local_zone: 1,
            local_net: 2,
            local_node: 3,
            local_point: 0,
            enabled: true,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000000Z".to_string(),
        }
    }

    fn local_area() -> crate::message_repo::MessageAreaRecord {
        crate::message_repo::MessageAreaRecord {
            id: LOCAL_AREA_ID.to_string(),
            key: "local".to_string(),
            name: "Local".to_string(),
            description: "local".to_string(),
            kind: "local".to_string(),
            network_id: None,
            read_security_level: 0,
            post_security_level: 10,
            moderated: false,
            enabled: true,
        }
    }

    fn insert_message_area(db: &Db) {
        crate::message_repo::insert_message_area(db, &local_area()).expect("insert local area");
    }

    fn link(profile_id: &str) -> NetworkLinkRecord {
        NetworkLinkRecord {
            id: LINK_ID.to_string(),
            key: "node-a".to_string(),
            network_id: profile_id.to_string(),
            address: "node@host".to_string(),
            host: "1.2.3.4".to_string(),
            binkp_port: 24554,
            password: "secret".to_string(),
            poll_schedule_minutes: 15,
            compression: "zip".to_string(),
            transport_security: "tls_required".to_string(),
            enabled: true,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000000Z".to_string(),
        }
    }

    fn area(profile_id: &str) -> NetworkAreaRecord {
        NetworkAreaRecord {
            id: AREA_ID.to_string(),
            network_id: profile_id.to_string(),
            area_tag: "TEST.ECHO".to_string(),
            local_area_id: LOCAL_AREA_ID.to_string(),
            description: "test".to_string(),
            read_only: false,
            subscribed: true,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            updated_at: "2026-01-01T00:00:00.000000Z".to_string(),
        }
    }

    fn packet(profile_id: &str, link_id: Option<&str>) -> NetworkPacketRecord {
        NetworkPacketRecord {
            id: PACKET_ID.to_string(),
            network_id: profile_id.to_string(),
            direction: "outbound".to_string(),
            link_id: link_id.map(|id| id.to_string()),
            filename: "pkg.pkt".to_string(),
            sha256: "abc".to_string(),
            size_bytes: 42,
            status: "pending".to_string(),
            error_message: None,
            received_at: None,
            processed_at: None,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
        }
    }

    fn second_packet(profile_id: &str, status: &str) -> NetworkPacketRecord {
        NetworkPacketRecord {
            id: SECOND_PACKET_ID.to_string(),
            network_id: profile_id.to_string(),
            direction: "inbound".to_string(),
            link_id: None,
            filename: "inbound.pkt".to_string(),
            sha256: "def".to_string(),
            size_bytes: 100,
            status: status.to_string(),
            error_message: (status == "failed").then(|| "bad packet".to_string()),
            received_at: Some("2026-01-01T00:00:00.000000Z".to_string()),
            processed_at: (status != "pending").then(|| "2026-01-01T00:00:01.000000Z".to_string()),
            created_at: "2026-01-01T00:00:01.000000Z".to_string(),
        }
    }

    fn message(profile_id: &str) -> NetworkMessageRecord {
        NetworkMessageRecord {
            id: MESSAGE_ID.to_string(),
            network_id: profile_id.to_string(),
            local_message_id: None,
            message_type: "echomail".to_string(),
            area_tag: Some("TEST.ECHO".to_string()),
            origin_address: "1:2/3.4".to_string(),
            destination_address: Some("1:2/3.5".to_string()),
            from_name: "Alice".to_string(),
            to_name: None,
            subject: "Hello".to_string(),
            raw_text: b"Raw body".to_vec(),
            display_body: "Hello".to_string(),
            msgid: Some("MSGID".to_string()),
            replyid: None,
            created_at: "2026-01-01T00:00:00.000000Z".to_string(),
            imported_at: None,
            exported_at: None,
            duplicate_hash: None,
            packet_id: None,
            status: "imported".to_string(),
        }
    }

    fn path_node(message_id: &str, network_id: &str, sequence: i64) -> NetworkPathNode {
        NetworkPathNode {
            id: format!(
                "00000000-0000-4000-8000-{:012x}",
                16 + u64::try_from(sequence).unwrap_or(0)
            ),
            message_id: message_id.to_string(),
            network_id: network_id.to_string(),
            sequence,
            zone: 1,
            net: 2,
            node: 3,
        }
    }

    fn seen(profile_id: &str, message_id: &str) -> NetworkSeenByNode {
        NetworkSeenByNode {
            id: "00000000-0000-4000-8000-100000000012".to_string(),
            message_id: message_id.to_string(),
            network_id: profile_id.to_string(),
            zone: 1,
            net: 2,
            node: 3,
        }
    }

    fn poll_log(link_id: &str) -> NetworkPollLogRecord {
        NetworkPollLogRecord {
            id: POLL_ID.to_string(),
            link_id: link_id.to_string(),
            started_at: "2026-01-01T00:00:00.000000Z".to_string(),
            ended_at: None,
            direction: "outbound".to_string(),
            status: "started".to_string(),
            bytes_in: 0,
            bytes_out: 0,
            packets_in: 0,
            packets_out: 0,
            error_message: None,
        }
    }

    fn subscription(area_id: &str, link_id: &str) -> NetworkSubscriptionRecord {
        NetworkSubscriptionRecord {
            id: SUBSCRIPTION_ID.to_string(),
            area_id: area_id.to_string(),
            link_id: link_id.to_string(),
            subscribed: true,
            subscribed_at: "2026-01-01T00:00:00.000000Z".to_string(),
            unsubscribed_at: None,
            source: "manual".to_string(),
        }
    }

    fn nodelist(profile_id: &str) -> NetworkNodelistRecord {
        NetworkNodelistRecord {
            id: NODELIST_ID.to_string(),
            network_id: profile_id.to_string(),
            zone: 1,
            net: 2,
            node: 3,
            point: 0,
            parsed_name: Some("Node".to_string()),
            raw_entry: "Zone:1".to_string(),
            updated_at: "2026-01-01T00:00:00.000000Z".to_string(),
        }
    }

    #[test]
    fn insert_and_find_network_profile() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");

        let found = find_network_profile_by_key(&db, PROFILE_KEY).expect("find profile");

        assert_eq!(found, Some(profile));
    }

    #[test]
    fn inserts_and_lists_network_link_and_area() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_message_area(&db);
        insert_network_link(&db, &link(&profile.id)).expect("insert link");
        insert_network_area(&db, &area(&profile.id)).expect("insert area");

        let links = list_network_links(&db).expect("list links");
        let areas = list_network_areas(&db).expect("list areas");

        assert_eq!(links.len(), 1);
        assert_eq!(areas.len(), 1);
        assert_eq!(areas[0].area_tag, "TEST.ECHO");
    }

    #[test]
    fn network_area_lookup_is_by_network_and_tag() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_message_area(&db);
        insert_network_area(&db, &area(&profile.id)).expect("insert area");

        let found =
            find_network_area_by_tag_and_profile(&db, &profile.id, "TEST.ECHO").expect("find area");
        assert_eq!(
            found.as_ref().map(|record| &record.id),
            Some(&AREA_ID.to_string())
        );
    }

    #[test]
    fn packet_finish_updates_status_and_timestamps() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_network_packet(&db, &packet(&profile.id, None)).expect("insert packet");

        finish_network_packet(&db, PACKET_ID, "processed", Some("ok")).expect("finish packet");

        let packets = list_network_packets(&db).expect("list packets");
        assert_eq!(packets.len(), 1);
        assert_eq!(packets[0].status, "processed");
    }

    #[test]
    fn packet_lookup_and_summary_are_queryable() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_network_packet(&db, &packet(&profile.id, None)).expect("insert packet");
        insert_network_packet(&db, &second_packet(&profile.id, "failed"))
            .expect("insert failed packet");

        let found = find_network_packet_by_id(&db, PACKET_ID).expect("find packet");
        let missing = find_network_packet_by_id(&db, "00000000-0000-4000-8000-999999999999")
            .expect("find missing packet");
        let summary = summarize_network_packets(&db, Some(&profile.id)).expect("summary");

        assert_eq!(
            found.as_ref().map(|packet| packet.filename.as_str()),
            Some("pkg.pkt")
        );
        assert!(missing.is_none());
        assert_eq!(
            summary,
            vec![
                NetworkPacketSummaryRecord {
                    direction: "inbound".to_string(),
                    status: "failed".to_string(),
                    count: 1,
                    total_size_bytes: 100,
                },
                NetworkPacketSummaryRecord {
                    direction: "outbound".to_string(),
                    status: "pending".to_string(),
                    count: 1,
                    total_size_bytes: 42,
                },
            ]
        );
    }

    #[test]
    fn packet_requeue_clears_error_and_processed_timestamp() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_network_packet(&db, &second_packet(&profile.id, "failed"))
            .expect("insert failed packet");

        assert!(requeue_network_packet(&db, SECOND_PACKET_ID).expect("requeue packet"));
        assert!(
            !requeue_network_packet(&db, "00000000-0000-4000-8000-999999999999")
                .expect("requeue missing packet")
        );

        let packet = find_network_packet_by_id(&db, SECOND_PACKET_ID)
            .expect("find packet")
            .expect("packet exists");
        assert_eq!(packet.status, "pending");
        assert_eq!(packet.error_message, None);
        assert_eq!(packet.processed_at, None);
    }

    #[test]
    fn packet_quarantine_sets_status_reason_and_processed_timestamp() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_network_packet(&db, &packet(&profile.id, None)).expect("insert packet");

        assert!(
            mark_network_packet_quarantined(&db, PACKET_ID, "operator review")
                .expect("quarantine packet")
        );

        let packet = find_network_packet_by_id(&db, PACKET_ID)
            .expect("find packet")
            .expect("packet exists");
        assert_eq!(packet.status, "quarantined");
        assert_eq!(packet.error_message.as_deref(), Some("operator review"));
        assert!(packet.processed_at.is_some());
    }

    #[test]
    fn message_records_preserve_raw_text_bytes() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_network_message(&db, &message(&profile.id)).expect("insert message");

        let messages = list_network_messages(&db).expect("list messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].raw_text, b"Raw body");
    }

    #[test]
    fn inserts_seen_by_and_path_with_transaction() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_message_area(&db);
        insert_network_message(&db, &message(&profile.id)).expect("insert message");
        let nodes = vec![
            path_node(MESSAGE_ID, &profile.id, 0),
            path_node(MESSAGE_ID, &profile.id, 1),
        ];
        insert_network_path(&db, &nodes).expect("insert path");
        insert_network_seen_by(&db, &seen(&profile.id, MESSAGE_ID)).expect("insert seen");

        let paths = list_network_path(&db).expect("list path");
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[1].sequence, 1);
    }

    #[test]
    fn duplicate_and_poll_logs_are_queryable() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_network_duplicate_log(
            &db,
            &NetworkDuplicateLogRecord {
                id: DUP_LOG_ID.to_string(),
                network_id: profile.id.clone(),
                duplicate_hash: "hash".to_string(),
                msgid: Some("MSGID".to_string()),
                area_tag: Some("TEST.ECHO".to_string()),
                origin_address: "1:2/3.4".to_string(),
                detected_at: "2026-01-01T00:00:00.000000Z".to_string(),
                action: "rejected".to_string(),
            },
        )
        .expect("insert duplicate log");

        let poll = poll_log(LINK_ID);
        let link = link(&profile.id);
        // link must exist for foreign-key validation.
        insert_message_area(&db);
        insert_network_link(&db, &link).expect("insert link");
        insert_network_poll_log(&db, &poll).expect("insert poll");
        finish_network_poll(&db, POLL_ID, "success", None).expect("finish poll");

        let duplicates = list_network_duplicates(&db).expect("list duplicates");
        let polls = list_network_poll_logs(&db).expect("list polls");
        assert_eq!(duplicates.len(), 1);
        assert_eq!(polls[0].status, "success");
    }

    #[test]
    fn inserts_subscriptions_and_nodelist_rows() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_message_area(&db);
        let created_link = link(&profile.id);
        let created_area = area(&profile.id);
        insert_network_link(&db, &created_link).expect("insert link");
        insert_network_area(&db, &created_area).expect("insert area");
        insert_network_subscription(&db, &subscription(&created_area.id, &created_link.id))
            .expect("insert subscription");
        insert_network_nodelist_entry(&db, &nodelist(&profile.id)).expect("insert nodelist");
        set_network_profile_enabled(&db, &profile.id, false).expect("disable profile");

        let subscriptions = list_network_subscriptions(&db).expect("list subscriptions");
        let nodes = list_network_nodelist_entries(&db).expect("list nodelist");
        let maybe_profile = find_network_profile_by_key(&db, PROFILE_KEY).expect("find profile");

        assert_eq!(subscriptions.len(), 1);
        assert_eq!(nodes.len(), 1);
        assert_eq!(maybe_profile.as_ref().map(|p| p.enabled), Some(false));
    }

    #[test]
    fn updates_area_and_link_subscription_status() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_message_area(&db);
        let created_link = link(&profile.id);
        let created_area = area(&profile.id);
        insert_network_link(&db, &created_link).expect("insert link");
        insert_network_area(&db, &created_area).expect("insert area");
        insert_network_subscription(&db, &subscription(&created_area.id, &created_link.id))
            .expect("insert subscription");

        assert!(
            set_network_subscription_status(
                &db,
                &created_area.id,
                &created_link.id,
                false,
                "2026-01-02T00:00:00.000000Z",
                "manual",
            )
            .expect("unsubscribe")
        );
        assert!(
            set_network_area_subscribed(&db, &created_area.id, false).expect("set area subscribed")
        );

        let subscriptions = list_network_subscriptions(&db).expect("list subscriptions");
        let areas = list_network_areas(&db).expect("list areas");
        assert!(!subscriptions[0].subscribed);
        assert_eq!(
            subscriptions[0].unsubscribed_at.as_deref(),
            Some("2026-01-02T00:00:00.000000Z")
        );
        assert!(!areas[0].subscribed);
    }

    #[test]
    fn replace_nodelist_entries_is_profile_scoped() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_network_nodelist_entry(&db, &nodelist(&profile.id)).expect("insert old nodelist");

        let replacement = NetworkNodelistRecord {
            id: "00000000-0000-4000-8000-100000000099".to_string(),
            network_id: profile.id.clone(),
            zone: 1,
            net: 2,
            node: 42,
            point: 7,
            parsed_name: Some("Point Node".to_string()),
            raw_entry: "Point,7,Point_Node".to_string(),
            updated_at: "2026-01-02T00:00:00.000000Z".to_string(),
        };

        replace_network_nodelist_entries(&db, &profile.id, std::slice::from_ref(&replacement))
            .expect("replace nodelist");

        let nodes = list_network_nodelist_entries(&db).expect("list nodelist");
        let found =
            find_network_nodelist_entry(&db, &profile.id, 1, 2, 42, 7).expect("find nodelist");

        assert_eq!(nodes, vec![replacement.clone()]);
        assert_eq!(found, Some(replacement));
    }

    #[test]
    fn insert_path_node_and_insert_seen_by_node_are_equivalent() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");
        insert_message_area(&db);
        insert_network_message(&db, &message(&profile.id)).expect("insert message");
        let node = path_node(MESSAGE_ID, &profile.id, 0);
        insert_network_path_node(&db, &node).expect("insert path node");
        insert_network_seen_by_node(&db, &seen(&profile.id, MESSAGE_ID))
            .expect("insert seen by node");

        let seen_by = list_network_seen_by(&db).expect("list seen by");
        let messages = list_network_messages(&db).expect("list messages");

        assert_eq!(seen_by.len(), 1);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn stress_test_50000_entry_nodelist() {
        let db = test_db();
        let profile = profile();
        insert_network_profile(&db, &profile).expect("insert profile");

        // Generate 50,000 nodelist entries
        let entries: Vec<NetworkNodelistRecord> = (0..50000)
            .map(|i| {
                let zone = 1;
                let net = (i / 1000) as i64 + 1;
                let node = (i % 1000) as i64 + 1;
                NetworkNodelistRecord {
                    id: format!("00000000-0000-4000-8000-{:012}", i),
                    network_id: profile.id.clone(),
                    zone,
                    net,
                    node,
                    point: 0,
                    parsed_name: Some(format!("Node_{}", i)),
                    raw_entry: format!("{},{},{},Node_{}", zone, net, node, i),
                    updated_at: "2026-06-04T00:00:00.000000Z".to_string(),
                }
            })
            .collect();

        let start = std::time::Instant::now();
        replace_network_nodelist_entries(&db, &profile.id, &entries).expect("replace nodelist");
        let insert_elapsed = start.elapsed();

        let start = std::time::Instant::now();
        let found = find_network_nodelist_entry(&db, &profile.id, 1, 25, 500, 0).expect("find");
        let query_elapsed = start.elapsed();

        assert!(found.is_some());
        let nodes = list_network_nodelist_entries(&db).expect("list nodelist");
        assert_eq!(nodes.len(), 50000);

        println!(
            "50k nodelist insert: {:?}, query: {:?}",
            insert_elapsed, query_elapsed
        );
    }
}
