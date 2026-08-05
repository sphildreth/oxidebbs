use decentdb::{Db, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OxideNetApplicationRecord {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub submitted_at: Option<String>,
    pub reviewed_at: Option<String>,
    pub status: String,
    pub applicant_user_id: Option<String>,
    pub board_name: String,
    pub sysop_alias: String,
    pub contact_email: String,
    pub host: String,
    pub binkp_port: i64,
    pub telnet_host: Option<String>,
    pub telnet_port: Option<i64>,
    pub software: String,
    pub software_version: String,
    pub timezone: String,
    pub region: String,
    pub description: String,
    pub reason: String,
    pub policy_version: String,
    pub policy_accepted_at: Option<String>,
    pub admin_notes: String,
    pub reviewed_by_user_id: Option<String>,
    pub assigned_address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OxideNetNodeRecord {
    pub id: String,
    pub application_id: Option<String>,
    pub network_key: String,
    pub address: String,
    pub zone: i64,
    pub net: i64,
    pub node: i64,
    pub point: i64,
    pub hub_address: String,
    pub board_name: String,
    pub sysop_alias: String,
    pub contact_email: String,
    pub host: String,
    pub binkp_port: i64,
    pub telnet_host: Option<String>,
    pub telnet_port: Option<i64>,
    pub software: String,
    pub software_version: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub activated_at: Option<String>,
    pub suspended_at: Option<String>,
    pub retired_at: Option<String>,
    pub last_poll_at: Option<String>,
    pub last_successful_poll_at: Option<String>,
    pub flags: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OxideNetCredentialRecord {
    pub id: String,
    pub node_id: String,
    pub credential_kind: String,
    pub secret_hash: String,
    pub created_at: String,
    pub rotated_at: Option<String>,
    pub expires_at: Option<String>,
    pub status: String,
}

pub fn insert_oxidenet_application(
    db: &Db,
    record: &OxideNetApplicationRecord,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_applications (
            id, created_at, updated_at, submitted_at, reviewed_at, status,
            applicant_user_id, board_name, sysop_alias, contact_email, host,
            binkp_port, telnet_host, telnet_port, software, software_version,
            timezone, region, description, reason, policy_version,
            policy_accepted_at, admin_notes, reviewed_by_user_id,
            assigned_address
        )
        VALUES (
            UUID_PARSE($1), $2, $3, $4, $5, $6, UUID_PARSE($7), $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22,
            $23, UUID_PARSE($24), $25
        )",
        &[
            Value::Text(record.id.clone()),
            Value::Text(record.created_at.clone()),
            Value::Text(record.updated_at.clone()),
            opt_text(&record.submitted_at),
            opt_text(&record.reviewed_at),
            Value::Text(record.status.clone()),
            opt_text(&record.applicant_user_id),
            Value::Text(record.board_name.clone()),
            Value::Text(record.sysop_alias.clone()),
            Value::Text(record.contact_email.clone()),
            Value::Text(record.host.clone()),
            Value::Int64(record.binkp_port),
            opt_text(&record.telnet_host),
            opt_int(record.telnet_port),
            Value::Text(record.software.clone()),
            Value::Text(record.software_version.clone()),
            Value::Text(record.timezone.clone()),
            Value::Text(record.region.clone()),
            Value::Text(record.description.clone()),
            Value::Text(record.reason.clone()),
            Value::Text(record.policy_version.clone()),
            opt_text(&record.policy_accepted_at),
            Value::Text(record.admin_notes.clone()),
            opt_text(&record.reviewed_by_user_id),
            opt_text(&record.assigned_address),
        ],
    )?;
    Ok(())
}

pub fn find_oxidenet_application_by_id(
    db: &Db,
    id: &str,
) -> decentdb::Result<Option<OxideNetApplicationRecord>> {
    let result = db.execute_with_params(
        "SELECT
            UUID_TO_STRING(id), CAST(created_at AS TEXT), CAST(updated_at AS TEXT),
            CAST(submitted_at AS TEXT), CAST(reviewed_at AS TEXT), status,
            UUID_TO_STRING(applicant_user_id), board_name, sysop_alias,
            contact_email, host, binkp_port, telnet_host, telnet_port, software,
            software_version, timezone, region, description, reason,
            policy_version, CAST(policy_accepted_at AS TEXT), admin_notes,
            UUID_TO_STRING(reviewed_by_user_id), assigned_address
         FROM network_applications
         WHERE id = UUID_PARSE($1)",
        &[Value::Text(id.to_string())],
    )?;
    Ok(result.rows().first().map(application_from_row))
}

pub fn list_oxidenet_applications(
    db: &Db,
    limit: i64,
) -> decentdb::Result<Vec<OxideNetApplicationRecord>> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let result = db.execute_with_params(
        "SELECT
            UUID_TO_STRING(id), CAST(created_at AS TEXT), CAST(updated_at AS TEXT),
            CAST(submitted_at AS TEXT), CAST(reviewed_at AS TEXT), status,
            UUID_TO_STRING(applicant_user_id), board_name, sysop_alias,
            contact_email, host, binkp_port, telnet_host, telnet_port, software,
            software_version, timezone, region, description, reason,
            policy_version, CAST(policy_accepted_at AS TEXT), admin_notes,
            UUID_TO_STRING(reviewed_by_user_id), assigned_address
         FROM network_applications
         ORDER BY created_at DESC
         LIMIT $1",
        &[Value::Int64(limit)],
    )?;
    Ok(result.rows().iter().map(application_from_row).collect())
}

pub fn update_oxidenet_application_status(
    db: &Db,
    id: &str,
    status: &str,
    reviewed_at: Option<&str>,
    reviewed_by_user_id: Option<&str>,
    admin_notes: Option<&str>,
    assigned_address: Option<&str>,
) -> decentdb::Result<bool> {
    let result = db.execute_with_params(
        "UPDATE network_applications
         SET status = $2,
             reviewed_at = $3,
             reviewed_by_user_id = UUID_PARSE($4),
             admin_notes = COALESCE($5, admin_notes),
             assigned_address = $6,
             updated_at = CURRENT_TIMESTAMP
         WHERE id = UUID_PARSE($1)",
        &[
            Value::Text(id.to_string()),
            Value::Text(status.to_string()),
            opt_str(reviewed_at),
            opt_str(reviewed_by_user_id),
            opt_str(admin_notes),
            opt_str(assigned_address),
        ],
    )?;
    Ok(result.affected_rows() > 0)
}

pub fn insert_oxidenet_node(db: &Db, record: &OxideNetNodeRecord) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_nodes (
            id, application_id, network_key, address, zone, net, node, point,
            hub_address, board_name, sysop_alias, contact_email, host,
            binkp_port, telnet_host, telnet_port, software, software_version,
            status, created_at, updated_at, activated_at, suspended_at,
            retired_at, last_poll_at, last_successful_poll_at, flags
        )
        VALUES (
            UUID_PARSE($1), UUID_PARSE($2), $3, $4, $5, $6, $7, $8, $9, $10,
            $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23,
            $24, $25, $26, $27
        )",
        &[
            Value::Text(record.id.clone()),
            opt_text(&record.application_id),
            Value::Text(record.network_key.clone()),
            Value::Text(record.address.clone()),
            Value::Int64(record.zone),
            Value::Int64(record.net),
            Value::Int64(record.node),
            Value::Int64(record.point),
            Value::Text(record.hub_address.clone()),
            Value::Text(record.board_name.clone()),
            Value::Text(record.sysop_alias.clone()),
            Value::Text(record.contact_email.clone()),
            Value::Text(record.host.clone()),
            Value::Int64(record.binkp_port),
            opt_text(&record.telnet_host),
            opt_int(record.telnet_port),
            Value::Text(record.software.clone()),
            Value::Text(record.software_version.clone()),
            Value::Text(record.status.clone()),
            Value::Text(record.created_at.clone()),
            Value::Text(record.updated_at.clone()),
            opt_text(&record.activated_at),
            opt_text(&record.suspended_at),
            opt_text(&record.retired_at),
            opt_text(&record.last_poll_at),
            opt_text(&record.last_successful_poll_at),
            Value::Text(record.flags.clone()),
        ],
    )?;
    Ok(())
}

pub fn find_oxidenet_node_by_address(
    db: &Db,
    address: &str,
) -> decentdb::Result<Option<OxideNetNodeRecord>> {
    let result = db.execute_with_params(
        "SELECT
            UUID_TO_STRING(id), UUID_TO_STRING(application_id), network_key,
            address, zone, net, node, point, hub_address, board_name,
            sysop_alias, contact_email, host, binkp_port, telnet_host,
            telnet_port, software, software_version, status,
            CAST(created_at AS TEXT), CAST(updated_at AS TEXT),
            CAST(activated_at AS TEXT), CAST(suspended_at AS TEXT),
            CAST(retired_at AS TEXT), CAST(last_poll_at AS TEXT),
            CAST(last_successful_poll_at AS TEXT), flags
         FROM network_nodes
         WHERE address = $1",
        &[Value::Text(address.to_string())],
    )?;
    Ok(result.rows().first().map(node_from_row))
}

pub fn list_oxidenet_nodes(db: &Db, limit: i64) -> decentdb::Result<Vec<OxideNetNodeRecord>> {
    if limit <= 0 {
        return Ok(Vec::new());
    }

    let result = db.execute_with_params(
        "SELECT
            UUID_TO_STRING(id), UUID_TO_STRING(application_id), network_key,
            address, zone, net, node, point, hub_address, board_name,
            sysop_alias, contact_email, host, binkp_port, telnet_host,
            telnet_port, software, software_version, status,
            CAST(created_at AS TEXT), CAST(updated_at AS TEXT),
            CAST(activated_at AS TEXT), CAST(suspended_at AS TEXT),
            CAST(retired_at AS TEXT), CAST(last_poll_at AS TEXT),
            CAST(last_successful_poll_at AS TEXT), flags
         FROM network_nodes
         ORDER BY zone, net, node, point
         LIMIT $1",
        &[Value::Int64(limit)],
    )?;
    Ok(result.rows().iter().map(node_from_row).collect())
}

pub fn update_oxidenet_node_status(
    db: &Db,
    node_id: &str,
    status: &str,
    timestamp: &str,
) -> decentdb::Result<bool> {
    let timestamp_column = match status {
        "active" => "activated_at",
        "suspended" => "suspended_at",
        "retired" => "retired_at",
        _ => "updated_at",
    };
    let sql = format!(
        "UPDATE network_nodes
         SET status = $2, updated_at = CURRENT_TIMESTAMP, {timestamp_column} = $3
         WHERE id = UUID_PARSE($1)"
    );
    let result = db.execute_with_params(
        &sql,
        &[
            Value::Text(node_id.to_string()),
            Value::Text(status.to_string()),
            Value::Text(timestamp.to_string()),
        ],
    )?;
    Ok(result.affected_rows() > 0)
}

pub fn record_oxidenet_node_poll(
    db: &Db,
    node_id: &str,
    polled_at: &str,
    successful: bool,
) -> decentdb::Result<bool> {
    let sql = if successful {
        "UPDATE network_nodes
         SET last_poll_at = $2, last_successful_poll_at = $2, updated_at = CURRENT_TIMESTAMP
         WHERE id = UUID_PARSE($1)"
    } else {
        "UPDATE network_nodes
         SET last_poll_at = $2, updated_at = CURRENT_TIMESTAMP
         WHERE id = UUID_PARSE($1)"
    };
    let result = db.execute_with_params(
        sql,
        &[
            Value::Text(node_id.to_string()),
            Value::Text(polled_at.to_string()),
        ],
    )?;
    Ok(result.affected_rows() > 0)
}

pub fn insert_oxidenet_credential(
    db: &Db,
    record: &OxideNetCredentialRecord,
) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO network_credentials (
            id, node_id, credential_kind, secret_hash, created_at, rotated_at,
            expires_at, status
        )
        VALUES (UUID_PARSE($1), UUID_PARSE($2), $3, $4, $5, $6, $7, $8)",
        &[
            Value::Text(record.id.clone()),
            Value::Text(record.node_id.clone()),
            Value::Text(record.credential_kind.clone()),
            Value::Text(record.secret_hash.clone()),
            Value::Text(record.created_at.clone()),
            opt_text(&record.rotated_at),
            opt_text(&record.expires_at),
            Value::Text(record.status.clone()),
        ],
    )?;
    Ok(())
}

pub fn list_oxidenet_credentials_for_node(
    db: &Db,
    node_id: &str,
) -> decentdb::Result<Vec<OxideNetCredentialRecord>> {
    let result = db.execute_with_params(
        "SELECT
            UUID_TO_STRING(id), UUID_TO_STRING(node_id), credential_kind,
            secret_hash, CAST(created_at AS TEXT), CAST(rotated_at AS TEXT),
            CAST(expires_at AS TEXT), status
         FROM network_credentials
         WHERE node_id = UUID_PARSE($1)
         ORDER BY created_at DESC",
        &[Value::Text(node_id.to_string())],
    )?;
    Ok(result.rows().iter().map(credential_from_row).collect())
}

pub fn revoke_oxidenet_credential(
    db: &Db,
    credential_id: &str,
    rotated_at: &str,
) -> decentdb::Result<bool> {
    let result = db.execute_with_params(
        "UPDATE network_credentials
         SET status = 'revoked', rotated_at = $2
         WHERE id = UUID_PARSE($1)",
        &[
            Value::Text(credential_id.to_string()),
            Value::Text(rotated_at.to_string()),
        ],
    )?;
    Ok(result.affected_rows() > 0)
}

fn application_from_row(row: &decentdb::QueryRow) -> OxideNetApplicationRecord {
    let values = row.values();
    OxideNetApplicationRecord {
        id: text_value(&values[0]),
        created_at: text_value(&values[1]),
        updated_at: text_value(&values[2]),
        submitted_at: opt_text_value(&values[3]),
        reviewed_at: opt_text_value(&values[4]),
        status: text_value(&values[5]),
        applicant_user_id: opt_text_value(&values[6]),
        board_name: text_value(&values[7]),
        sysop_alias: text_value(&values[8]),
        contact_email: text_value(&values[9]),
        host: text_value(&values[10]),
        binkp_port: int_value(&values[11]),
        telnet_host: opt_text_value(&values[12]),
        telnet_port: opt_int_value(&values[13]),
        software: text_value(&values[14]),
        software_version: text_value(&values[15]),
        timezone: text_value(&values[16]),
        region: text_value(&values[17]),
        description: text_value(&values[18]),
        reason: text_value(&values[19]),
        policy_version: text_value(&values[20]),
        policy_accepted_at: opt_text_value(&values[21]),
        admin_notes: text_value(&values[22]),
        reviewed_by_user_id: opt_text_value(&values[23]),
        assigned_address: opt_text_value(&values[24]),
    }
}

fn node_from_row(row: &decentdb::QueryRow) -> OxideNetNodeRecord {
    let values = row.values();
    OxideNetNodeRecord {
        id: text_value(&values[0]),
        application_id: opt_text_value(&values[1]),
        network_key: text_value(&values[2]),
        address: text_value(&values[3]),
        zone: int_value(&values[4]),
        net: int_value(&values[5]),
        node: int_value(&values[6]),
        point: int_value(&values[7]),
        hub_address: text_value(&values[8]),
        board_name: text_value(&values[9]),
        sysop_alias: text_value(&values[10]),
        contact_email: text_value(&values[11]),
        host: text_value(&values[12]),
        binkp_port: int_value(&values[13]),
        telnet_host: opt_text_value(&values[14]),
        telnet_port: opt_int_value(&values[15]),
        software: text_value(&values[16]),
        software_version: text_value(&values[17]),
        status: text_value(&values[18]),
        created_at: text_value(&values[19]),
        updated_at: text_value(&values[20]),
        activated_at: opt_text_value(&values[21]),
        suspended_at: opt_text_value(&values[22]),
        retired_at: opt_text_value(&values[23]),
        last_poll_at: opt_text_value(&values[24]),
        last_successful_poll_at: opt_text_value(&values[25]),
        flags: text_value(&values[26]),
    }
}

fn credential_from_row(row: &decentdb::QueryRow) -> OxideNetCredentialRecord {
    let values = row.values();
    OxideNetCredentialRecord {
        id: text_value(&values[0]),
        node_id: text_value(&values[1]),
        credential_kind: text_value(&values[2]),
        secret_hash: text_value(&values[3]),
        created_at: text_value(&values[4]),
        rotated_at: opt_text_value(&values[5]),
        expires_at: opt_text_value(&values[6]),
        status: text_value(&values[7]),
    }
}

fn opt_text(value: &Option<String>) -> Value {
    value
        .as_ref()
        .map(|value| Value::Text(value.clone()))
        .unwrap_or(Value::Null)
}

fn opt_str(value: Option<&str>) -> Value {
    value
        .map(|value| Value::Text(value.to_string()))
        .unwrap_or(Value::Null)
}

fn opt_int(value: Option<i64>) -> Value {
    value.map(Value::Int64).unwrap_or(Value::Null)
}

fn text_value(value: &Value) -> String {
    match value {
        Value::Text(text) => text.clone(),
        _ => String::new(),
    }
}

fn opt_text_value(value: &Value) -> Option<String> {
    match value {
        Value::Text(text) if !text.is_empty() => Some(text.clone()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema;
    use decentdb::DbConfig;

    const APP_ID: &str = "00000000-0000-4000-8000-200000000001";
    const NODE_ID: &str = "00000000-0000-4000-8000-200000000002";
    const CREDENTIAL_ID: &str = "00000000-0000-4000-8000-200000000003";
    const CREATED_AT: &str = "2026-06-04T00:00:00.000000Z";

    fn test_db() -> Db {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        schema::init_schema(&db).expect("init schema");
        db
    }

    fn application() -> OxideNetApplicationRecord {
        OxideNetApplicationRecord {
            id: APP_ID.to_string(),
            created_at: CREATED_AT.to_string(),
            updated_at: CREATED_AT.to_string(),
            submitted_at: Some(CREATED_AT.to_string()),
            reviewed_at: None,
            status: "submitted".to_string(),
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
            policy_accepted_at: Some(CREATED_AT.to_string()),
            admin_notes: String::new(),
            reviewed_by_user_id: None,
            assigned_address: None,
        }
    }

    fn node() -> OxideNetNodeRecord {
        OxideNetNodeRecord {
            id: NODE_ID.to_string(),
            application_id: Some(APP_ID.to_string()),
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
            status: "first-poll-pending".to_string(),
            created_at: CREATED_AT.to_string(),
            updated_at: CREATED_AT.to_string(),
            activated_at: None,
            suspended_at: None,
            retired_at: None,
            last_poll_at: None,
            last_successful_poll_at: None,
            flags: "CM".to_string(),
        }
    }

    fn credential() -> OxideNetCredentialRecord {
        OxideNetCredentialRecord {
            id: CREDENTIAL_ID.to_string(),
            node_id: NODE_ID.to_string(),
            credential_kind: "binkp_session".to_string(),
            secret_hash: "sha256:abc123".to_string(),
            created_at: CREATED_AT.to_string(),
            rotated_at: None,
            expires_at: None,
            status: "active".to_string(),
        }
    }

    #[test]
    fn insert_list_and_find_application() {
        let db = test_db();
        insert_oxidenet_application(&db, &application()).expect("insert application");

        let applications = list_oxidenet_applications(&db, 10).expect("list applications");
        assert_eq!(applications.len(), 1);
        assert_eq!(applications[0].board_name, "Example BBS");

        let found = find_oxidenet_application_by_id(&db, APP_ID)
            .expect("find application")
            .expect("application exists");
        assert_eq!(found.status, "submitted");
        assert_eq!(found.assigned_address, None);
    }

    #[test]
    fn update_application_status_records_assignment() {
        let db = test_db();
        insert_oxidenet_application(&db, &application()).expect("insert application");

        let updated = update_oxidenet_application_status(
            &db,
            APP_ID,
            "approved",
            Some("2026-06-04T01:00:00.000000Z"),
            None,
            Some("assigned"),
            Some("777:1/100"),
        )
        .expect("update application");

        assert!(updated);
        let found = find_oxidenet_application_by_id(&db, APP_ID)
            .expect("find application")
            .expect("application exists");
        assert_eq!(found.status, "approved");
        assert_eq!(found.admin_notes, "assigned");
        assert_eq!(found.assigned_address.as_deref(), Some("777:1/100"));
    }

    #[test]
    fn insert_list_and_find_node() {
        let db = test_db();
        insert_oxidenet_application(&db, &application()).expect("insert application");
        insert_oxidenet_node(&db, &node()).expect("insert node");

        let nodes = list_oxidenet_nodes(&db, 10).expect("list nodes");
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].address, "777:1/100");

        let found = find_oxidenet_node_by_address(&db, "777:1/100")
            .expect("find node")
            .expect("node exists");
        assert_eq!(found.application_id.as_deref(), Some(APP_ID));
        assert_eq!(found.status, "first-poll-pending");
    }

    #[test]
    fn node_state_helpers_update_lifecycle_fields() {
        let db = test_db();
        insert_oxidenet_application(&db, &application()).expect("insert application");
        insert_oxidenet_node(&db, &node()).expect("insert node");

        assert!(
            update_oxidenet_node_status(&db, NODE_ID, "active", "2026-06-04T02:00:00.000000Z")
                .expect("activate node")
        );
        assert!(
            record_oxidenet_node_poll(&db, NODE_ID, "2026-06-04T02:30:00.000000Z", true)
                .expect("record poll")
        );

        let found = find_oxidenet_node_by_address(&db, "777:1/100")
            .expect("find node")
            .expect("node exists");
        assert_eq!(found.status, "active");
        assert_eq!(
            found.activated_at.as_deref(),
            Some("2026-06-04T02:00:00.000000Z")
        );
        assert_eq!(
            found.last_poll_at.as_deref(),
            Some("2026-06-04T02:30:00.000000Z")
        );
        assert_eq!(
            found.last_successful_poll_at.as_deref(),
            Some("2026-06-04T02:30:00.000000Z")
        );
    }

    #[test]
    fn credentials_store_hashes_and_can_be_revoked() {
        let db = test_db();
        insert_oxidenet_application(&db, &application()).expect("insert application");
        insert_oxidenet_node(&db, &node()).expect("insert node");
        insert_oxidenet_credential(&db, &credential()).expect("insert credential");

        let credentials =
            list_oxidenet_credentials_for_node(&db, NODE_ID).expect("list credentials");
        assert_eq!(credentials.len(), 1);
        assert_eq!(credentials[0].credential_kind, "binkp_session");
        assert_eq!(credentials[0].secret_hash, "sha256:abc123");

        assert!(
            revoke_oxidenet_credential(&db, CREDENTIAL_ID, "2026-06-04T03:00:00.000000Z")
                .expect("revoke credential")
        );
        let credentials =
            list_oxidenet_credentials_for_node(&db, NODE_ID).expect("list credentials");
        assert_eq!(credentials[0].status, "revoked");
        assert_eq!(
            credentials[0].rotated_at.as_deref(),
            Some("2026-06-04T03:00:00.000000Z")
        );
    }
}
