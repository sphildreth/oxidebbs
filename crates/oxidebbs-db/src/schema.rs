use std::cmp::Ordering;

use decentdb::{Db, DbError, Value};

use super::SCHEMA_VERSION;
use super::migrations;

pub fn init_schema(db: &Db) -> decentdb::Result<()> {
    let version = existing_schema_version(db)?;

    match version {
        Some(version) => match version.cmp(&SCHEMA_VERSION) {
            Ordering::Less => {
                migrations::migrate_to_current(db)?;
                create_full_schema(db)
            }
            Ordering::Equal => Ok(()),
            Ordering::Greater => Err(DbError::sql(format!(
                "OxideBBS database schema version {version} is newer than supported version {SCHEMA_VERSION}"
            ))),
        },
        None => {
            if has_system_config_table(db)? {
                Err(DbError::sql(
                    "OxideBBS system_config table exists but schema_version marker is missing",
                ))
            } else if has_any_user_table(db)? {
                Err(DbError::sql(
                    "OxideBBS schema_version marker is missing; found existing database tables",
                ))
            } else {
                create_full_schema(db)
            }
        }
    }
}

fn has_any_user_table(db: &Db) -> decentdb::Result<bool> {
    Ok(!db.list_tables()?.is_empty())
}

fn has_system_config_table(db: &Db) -> decentdb::Result<bool> {
    Ok(db
        .list_tables()?
        .iter()
        .any(|table| table.name == "system_config"))
}

pub(crate) fn existing_schema_version(db: &Db) -> decentdb::Result<Option<i64>> {
    if !has_system_config_table(db)? {
        return Ok(None);
    }

    let result = db.execute_with_params(
        "SELECT value FROM system_config WHERE key = $1",
        &[Value::Text("schema_version".to_string())],
    )?;
    let Some(value) = result.rows().first().and_then(|row| row.values().first()) else {
        return Ok(None);
    };

    parse_schema_version(value).map(Some)
}

pub(crate) fn set_schema_version(db: &Db, version: i64) -> decentdb::Result<()> {
    db.execute_with_params(
        "INSERT INTO system_config (key, value)
         VALUES ($1, $2)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = CURRENT_TIMESTAMP",
        &[
            Value::Text("schema_version".to_string()),
            Value::Text(version.to_string()),
        ],
    )?;
    Ok(())
}

fn create_full_schema(db: &Db) -> decentdb::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS system_config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS users (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            alias TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(alias)) > 0),
            alias_normalized TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(alias_normalized)) > 0),
            real_name TEXT NOT NULL CHECK (LENGTH(TRIM(real_name)) > 0),
            email TEXT,
            password_hash TEXT NOT NULL,
            security_level INT NOT NULL DEFAULT 10 CHECK (security_level >= 0 AND security_level <= 255),
            is_sysop BOOL NOT NULL DEFAULT FALSE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_login_at TIMESTAMPTZ,
            total_calls INT NOT NULL DEFAULT 0 CHECK (total_calls >= 0),
            time_bank_minutes INT NOT NULL DEFAULT 0 CHECK (time_bank_minutes >= 0),
            status TEXT NOT NULL DEFAULT 'active'
                CHECK (status = 'active' OR status = 'locked' OR status = 'disabled')
        );

        CREATE TABLE IF NOT EXISTS auth_attempts (
            scope TEXT NOT NULL CHECK (scope = 'ip' OR scope = 'alias'),
            scope_key TEXT NOT NULL CHECK (LENGTH(TRIM(scope_key)) > 0),
            failed_count INT NOT NULL DEFAULT 0 CHECK (failed_count >= 0),
            first_failed_at TIMESTAMPTZ,
            last_failed_at TIMESTAMPTZ,
            locked_until TIMESTAMPTZ,
            PRIMARY KEY (scope, scope_key)
        );

        CREATE TABLE IF NOT EXISTS audit_events (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            event_type TEXT NOT NULL CHECK (LENGTH(TRIM(event_type)) > 0),
            user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            node_number INT CHECK (node_number IS NULL OR node_number > 0),
            details TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS message_areas (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            key TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(key)) > 0),
            name TEXT NOT NULL CHECK (LENGTH(TRIM(name)) > 0),
            description TEXT NOT NULL DEFAULT '',
            kind TEXT NOT NULL DEFAULT 'local',
            network_id TEXT,
            read_security_level INT NOT NULL DEFAULT 0 CHECK (read_security_level >= 0 AND read_security_level <= 255),
            post_security_level INT NOT NULL DEFAULT 10 CHECK (post_security_level >= 0 AND post_security_level <= 255),
            moderated BOOL NOT NULL DEFAULT FALSE,
            enabled BOOL NOT NULL DEFAULT TRUE,
            CHECK (kind = 'local' OR kind = 'echomail' OR kind = 'netmail')
        );

        CREATE TABLE IF NOT EXISTS messages (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            area_id UUID NOT NULL REFERENCES message_areas(id) ON DELETE CASCADE,
            author_user_id UUID REFERENCES users(id) ON DELETE RESTRICT,
            to_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            subject TEXT NOT NULL CHECK (LENGTH(TRIM(subject)) > 0),
            body TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            reply_to_id UUID REFERENCES messages(id) ON DELETE SET NULL,
            network_message_id TEXT,
            author_kind TEXT NOT NULL DEFAULT 'local'
                CHECK (author_kind = 'local' OR author_kind = 'network' OR author_kind = 'system'),
            author_display_name TEXT NOT NULL DEFAULT '',
            author_network_address TEXT,
            visibility TEXT NOT NULL DEFAULT 'normal'
                CHECK (visibility = 'normal' OR visibility = 'deleted' OR visibility = 'hidden')
        );

        CREATE TABLE IF NOT EXISTS network_profiles (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            key TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(key)) > 0),
            name TEXT NOT NULL CHECK (LENGTH(TRIM(name)) > 0),
            adapter TEXT NOT NULL DEFAULT 'legacy-ftn'
                CHECK (adapter = 'legacy-ftn' OR adapter = 'oxidenet'),
            local_zone INT NOT NULL CHECK (local_zone > 0),
            local_net INT NOT NULL CHECK (local_net > 0),
            local_node INT NOT NULL CHECK (local_node > 0),
            local_point INT NOT NULL DEFAULT 0 CHECK (local_point >= 0),
            enabled BOOL NOT NULL DEFAULT TRUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS network_links (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            key TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(key)) > 0),
            network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE,
            address TEXT NOT NULL CHECK (LENGTH(TRIM(address)) > 0),
            host TEXT NOT NULL CHECK (LENGTH(TRIM(host)) > 0),
            binkp_port INT NOT NULL DEFAULT 24554 CHECK (binkp_port > 0 AND binkp_port <= 65535),
            password TEXT NOT NULL,
            poll_schedule_minutes INT NOT NULL DEFAULT 60 CHECK (poll_schedule_minutes > 0),
            compression TEXT NOT NULL DEFAULT 'zip'
                CHECK (compression = 'none' OR compression = 'zip' OR compression = 'arj'),
            transport_security TEXT NOT NULL DEFAULT 'tls_required'
                CHECK (transport_security = 'tls_required' OR transport_security = 'tls_opportunistic' OR transport_security = 'plaintext_legacy'),
            enabled BOOL NOT NULL DEFAULT TRUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            CHECK (network_id IS NOT NULL)
        );

        CREATE TABLE IF NOT EXISTS network_areas (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE,
            area_tag TEXT NOT NULL,
            local_area_id UUID NOT NULL REFERENCES message_areas(id) ON DELETE CASCADE,
            description TEXT NOT NULL DEFAULT '',
            read_only BOOL NOT NULL DEFAULT FALSE,
            subscribed BOOL NOT NULL DEFAULT TRUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            CHECK (LENGTH(TRIM(area_tag)) > 0),
            UNIQUE (network_id, area_tag),
            UNIQUE (network_id, local_area_id)
        );

        CREATE TABLE IF NOT EXISTS network_packets (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE,
            direction TEXT NOT NULL
                CHECK (direction = 'inbound' OR direction = 'outbound'),
            link_id UUID REFERENCES network_links(id) ON DELETE SET NULL,
            filename TEXT NOT NULL CHECK (LENGTH(TRIM(filename)) > 0),
            sha256 TEXT NOT NULL CHECK (LENGTH(TRIM(sha256)) > 0),
            size_bytes INT NOT NULL DEFAULT 0 CHECK (size_bytes >= 0),
            status TEXT NOT NULL DEFAULT 'pending'
                CHECK (status = 'pending' OR status = 'processing' OR status = 'processed' OR status = 'quarantined' OR status = 'failed' OR status = 'ready'),
            error_message TEXT,
            received_at TIMESTAMPTZ,
            processed_at TIMESTAMPTZ,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS network_messages (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE,
            local_message_id UUID REFERENCES messages(id) ON DELETE SET NULL,
            message_type TEXT NOT NULL DEFAULT 'echomail'
                CHECK (message_type = 'echomail' OR message_type = 'netmail' OR message_type = 'local'),
            area_tag TEXT,
            origin_address TEXT NOT NULL,
            destination_address TEXT,
            from_name TEXT NOT NULL CHECK (LENGTH(TRIM(from_name)) > 0),
            to_name TEXT,
            subject TEXT NOT NULL CHECK (LENGTH(TRIM(subject)) > 0),
            raw_text BLOB NOT NULL,
            display_body TEXT NOT NULL DEFAULT '',
            msgid TEXT,
            replyid TEXT,
            created_at TIMESTAMPTZ NOT NULL,
            imported_at TIMESTAMPTZ,
            exported_at TIMESTAMPTZ,
            duplicate_hash TEXT,
            packet_id UUID REFERENCES network_packets(id) ON DELETE SET NULL,
            status TEXT NOT NULL DEFAULT 'imported'
                CHECK (status = 'imported' OR status = 'exported' OR status = 'quarantined' OR status = 'duplicate' OR status = 'pending' OR status = 'ready'),
            CHECK (LENGTH(TRIM(origin_address)) > 0)
        );

        CREATE TABLE IF NOT EXISTS network_seen_by (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            message_id UUID NOT NULL REFERENCES network_messages(id) ON DELETE CASCADE,
            network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE,
            zone INT NOT NULL CHECK (zone > 0),
            net INT NOT NULL CHECK (net > 0),
            node INT NOT NULL CHECK (node > 0)
        );

        CREATE TABLE IF NOT EXISTS network_path (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            message_id UUID NOT NULL REFERENCES network_messages(id) ON DELETE CASCADE,
            network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE,
            sequence INT NOT NULL CHECK (sequence >= 0),
            zone INT NOT NULL CHECK (zone > 0),
            net INT NOT NULL CHECK (net > 0),
            node INT NOT NULL CHECK (node > 0)
        );

        CREATE TABLE IF NOT EXISTS network_duplicate_log (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE,
            duplicate_hash TEXT NOT NULL,
            msgid TEXT,
            area_tag TEXT,
            origin_address TEXT NOT NULL,
            detected_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            action TEXT NOT NULL DEFAULT 'rejected'
                CHECK (action = 'rejected' OR action = 'quarantined' OR action = 'replaced'),
            CHECK (LENGTH(TRIM(duplicate_hash)) > 0)
        );

        CREATE TABLE IF NOT EXISTS network_poll_log (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            link_id UUID NOT NULL REFERENCES network_links(id) ON DELETE CASCADE,
            started_at TIMESTAMPTZ NOT NULL,
            ended_at TIMESTAMPTZ,
            direction TEXT NOT NULL
                CHECK (direction = 'inbound' OR direction = 'outbound' OR direction = 'bidirectional'),
            status TEXT NOT NULL DEFAULT 'started'
                CHECK (status = 'started' OR status = 'success' OR status = 'failed' OR status = 'timeout'),
            bytes_in INT NOT NULL DEFAULT 0 CHECK (bytes_in >= 0),
            bytes_out INT NOT NULL DEFAULT 0 CHECK (bytes_out >= 0),
            packets_in INT NOT NULL DEFAULT 0 CHECK (packets_in >= 0),
            packets_out INT NOT NULL DEFAULT 0 CHECK (packets_out >= 0),
            error_message TEXT
        );

        CREATE TABLE IF NOT EXISTS network_area_subscriptions (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            area_id UUID NOT NULL REFERENCES network_areas(id) ON DELETE CASCADE,
            link_id UUID NOT NULL REFERENCES network_links(id) ON DELETE CASCADE,
            subscribed BOOL NOT NULL DEFAULT TRUE,
            subscribed_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            unsubscribed_at TIMESTAMPTZ,
            source TEXT NOT NULL DEFAULT 'manual'
                CHECK (source = 'manual' OR source = 'areafix' OR source = 'default'),
            UNIQUE (area_id, link_id)
        );

        CREATE TABLE IF NOT EXISTS network_nodelist (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE,
            zone INT NOT NULL CHECK (zone > 0),
            net INT NOT NULL CHECK (net > 0),
            node INT NOT NULL CHECK (node > 0),
            point INT NOT NULL DEFAULT 0 CHECK (point >= 0),
            parsed_name TEXT,
            location TEXT,
            sysop_name TEXT,
            phone TEXT,
            speed TEXT,
            flags TEXT NOT NULL DEFAULT '',
            raw_entry TEXT NOT NULL CHECK (LENGTH(TRIM(raw_entry)) > 0),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (network_id, zone, net, node, point)
        );

        CREATE TABLE IF NOT EXISTS network_applications (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            submitted_at TIMESTAMPTZ,
            reviewed_at TIMESTAMPTZ,
            status TEXT NOT NULL DEFAULT 'submitted'
                CHECK (status = 'draft' OR status = 'submitted' OR status = 'needs-info' OR status = 'approved' OR status = 'config-generated' OR status = 'first-poll-pending' OR status = 'active' OR status = 'probation' OR status = 'suspended' OR status = 'retired' OR status = 'rejected' OR status = 'withdrawn' OR status = 'needs-review-hold'),
            applicant_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            board_name TEXT NOT NULL CHECK (LENGTH(TRIM(board_name)) > 0),
            sysop_alias TEXT NOT NULL CHECK (LENGTH(TRIM(sysop_alias)) > 0),
            contact_email TEXT NOT NULL CHECK (LENGTH(TRIM(contact_email)) > 0),
            host TEXT NOT NULL CHECK (LENGTH(TRIM(host)) > 0),
            binkp_port INT NOT NULL DEFAULT 24554 CHECK (binkp_port > 0 AND binkp_port <= 65535),
            telnet_host TEXT,
            telnet_port INT CHECK (telnet_port IS NULL OR (telnet_port > 0 AND telnet_port <= 65535)),
            software TEXT NOT NULL DEFAULT 'OxideBBS' CHECK (LENGTH(TRIM(software)) > 0),
            software_version TEXT NOT NULL DEFAULT '',
            timezone TEXT NOT NULL DEFAULT 'UTC' CHECK (LENGTH(TRIM(timezone)) > 0),
            region TEXT NOT NULL DEFAULT '',
            description TEXT NOT NULL DEFAULT '',
            reason TEXT NOT NULL DEFAULT '',
            policy_version TEXT NOT NULL DEFAULT '',
            policy_accepted_at TIMESTAMPTZ,
            admin_notes TEXT NOT NULL DEFAULT '',
            reviewed_by_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            assigned_address TEXT UNIQUE
        );

        CREATE TABLE IF NOT EXISTS network_nodes (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            application_id UUID REFERENCES network_applications(id) ON DELETE SET NULL,
            network_key TEXT NOT NULL DEFAULT 'oxidenet' CHECK (LENGTH(TRIM(network_key)) > 0),
            address TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(address)) > 0),
            zone INT NOT NULL CHECK (zone > 0),
            net INT NOT NULL CHECK (net > 0),
            node INT NOT NULL CHECK (node > 0),
            point INT NOT NULL DEFAULT 0 CHECK (point >= 0),
            hub_address TEXT NOT NULL CHECK (LENGTH(TRIM(hub_address)) > 0),
            board_name TEXT NOT NULL CHECK (LENGTH(TRIM(board_name)) > 0),
            sysop_alias TEXT NOT NULL CHECK (LENGTH(TRIM(sysop_alias)) > 0),
            contact_email TEXT NOT NULL CHECK (LENGTH(TRIM(contact_email)) > 0),
            host TEXT NOT NULL CHECK (LENGTH(TRIM(host)) > 0),
            binkp_port INT NOT NULL DEFAULT 24554 CHECK (binkp_port > 0 AND binkp_port <= 65535),
            telnet_host TEXT,
            telnet_port INT CHECK (telnet_port IS NULL OR (telnet_port > 0 AND telnet_port <= 65535)),
            software TEXT NOT NULL DEFAULT 'OxideBBS' CHECK (LENGTH(TRIM(software)) > 0),
            software_version TEXT NOT NULL DEFAULT '',
            status TEXT NOT NULL DEFAULT 'first-poll-pending'
                CHECK (status = 'first-poll-pending' OR status = 'config-generated' OR status = 'active' OR status = 'probation' OR status = 'suspended' OR status = 'retired'),
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            activated_at TIMESTAMPTZ,
            suspended_at TIMESTAMPTZ,
            retired_at TIMESTAMPTZ,
            last_poll_at TIMESTAMPTZ,
            last_successful_poll_at TIMESTAMPTZ,
            flags TEXT NOT NULL DEFAULT ''
        );

        CREATE TABLE IF NOT EXISTS network_credentials (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            node_id UUID NOT NULL REFERENCES network_nodes(id) ON DELETE CASCADE,
            credential_kind TEXT NOT NULL
                CHECK (credential_kind = 'binkp_session' OR credential_kind = 'invite_token'),
            secret_hash TEXT NOT NULL CHECK (LENGTH(TRIM(secret_hash)) > 0),
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            rotated_at TIMESTAMPTZ,
            expires_at TIMESTAMPTZ,
            status TEXT NOT NULL DEFAULT 'active'
                CHECK (status = 'active' OR status = 'revoked' OR status = 'expired')
        );

        CREATE TABLE IF NOT EXISTS network_rescan_queue (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            network_id UUID NOT NULL REFERENCES network_profiles(id) ON DELETE CASCADE,
            link_id UUID NOT NULL REFERENCES network_links(id) ON DELETE CASCADE,
            area_tag TEXT NOT NULL CHECK (LENGTH(TRIM(area_tag)) > 0),
            status TEXT NOT NULL DEFAULT 'pending'
                CHECK (status = 'pending' OR status = 'processing' OR status = 'completed' OR status = 'failed' OR status = 'cancelled'),
            requested_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            processed_at TIMESTAMPTZ
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            node_number INT NOT NULL CHECK (node_number > 0),
            user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            transport TEXT NOT NULL CHECK (transport = 'telnet' OR transport = 'serial' OR transport = 'websocket'),
            remote_address TEXT NOT NULL DEFAULT '',
            remote_ip IPADDR,
            remote_port INT CHECK (remote_port IS NULL OR (remote_port >= 0 AND remote_port <= 65535)),
            started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            ended_at TIMESTAMPTZ,
            disconnect_reason TEXT
        );

        CREATE TABLE IF NOT EXISTS doors (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            key TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(key)) > 0),
            name TEXT NOT NULL CHECK (LENGTH(TRIM(name)) > 0),
            runner TEXT NOT NULL CHECK (LENGTH(TRIM(runner)) > 0),
            working_dir TEXT NOT NULL CHECK (LENGTH(TRIM(working_dir)) > 0),
            command TEXT NOT NULL CHECK (LENGTH(TRIM(command)) > 0),
            drop_file TEXT NOT NULL CHECK (LENGTH(TRIM(drop_file)) > 0),
            exclusive BOOL NOT NULL DEFAULT FALSE,
            time_limit_minutes INT NOT NULL DEFAULT 30 CHECK (time_limit_minutes > 0),
            enabled BOOL NOT NULL DEFAULT TRUE,
            min_security_level INT NOT NULL DEFAULT 0 CHECK (min_security_level >= 0 AND min_security_level <= 255)
        );

        CREATE TABLE IF NOT EXISTS door_runs (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            door_id UUID NOT NULL REFERENCES doors(id) ON DELETE RESTRICT,
            user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
            node_number INT NOT NULL CHECK (node_number > 0),
            started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            ended_at TIMESTAMPTZ,
            exit_code INT,
            timed_out BOOL NOT NULL DEFAULT FALSE,
            disconnect_forced BOOL NOT NULL DEFAULT FALSE,
            bytes_in INT NOT NULL DEFAULT 0 CHECK (bytes_in >= 0),
            bytes_out INT NOT NULL DEFAULT 0 CHECK (bytes_out >= 0)
        );

        CREATE INDEX IF NOT EXISTS idx_audit_events_created_at ON audit_events (created_at);
        CREATE INDEX IF NOT EXISTS idx_audit_events_user_id ON audit_events (user_id);

        CREATE TABLE IF NOT EXISTS door_provider_credentials (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            door_id UUID NOT NULL REFERENCES doors(id) ON DELETE CASCADE,
            provider_name TEXT NOT NULL CHECK (LENGTH(TRIM(provider_name)) > 0),
            credential_ref TEXT NOT NULL CHECK (LENGTH(TRIM(credential_ref)) > 0),
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE (door_id, provider_name)
        );

        CREATE INDEX IF NOT EXISTS idx_door_provider_credentials_door_id ON door_provider_credentials (door_id);
        CREATE INDEX IF NOT EXISTS idx_messages_area_created_at ON messages (area_id, created_at);
        CREATE INDEX IF NOT EXISTS idx_messages_author_user_id ON messages (author_user_id);
        CREATE INDEX IF NOT EXISTS idx_messages_author_kind ON messages (author_kind);
        CREATE INDEX IF NOT EXISTS idx_messages_to_user_id ON messages (to_user_id);
        CREATE INDEX IF NOT EXISTS idx_network_profile_enabled ON network_profiles (enabled);
        CREATE INDEX IF NOT EXISTS idx_network_links_network_id_enabled ON network_links (network_id, enabled);
        CREATE INDEX IF NOT EXISTS idx_network_links_host ON network_links (host);
        CREATE INDEX IF NOT EXISTS idx_network_areas_network_id ON network_areas (network_id);
        CREATE INDEX IF NOT EXISTS idx_network_packets_network_id_status ON network_packets (network_id, status);
        CREATE INDEX IF NOT EXISTS idx_network_messages_network_id ON network_messages (network_id);
        CREATE INDEX IF NOT EXISTS idx_network_messages_local_message_id ON network_messages (local_message_id);
        CREATE INDEX IF NOT EXISTS idx_network_seen_by_message_id ON network_seen_by (message_id);
        CREATE INDEX IF NOT EXISTS idx_network_seen_by_network_id_zone_net_node ON network_seen_by (network_id, zone, net, node);
        CREATE INDEX IF NOT EXISTS idx_network_path_message_id_sequence ON network_path (message_id, sequence);
        CREATE INDEX IF NOT EXISTS idx_network_duplicate_log_detected_at ON network_duplicate_log (network_id, detected_at);
        CREATE INDEX IF NOT EXISTS idx_network_duplicate_log_duplicate_hash ON network_duplicate_log (duplicate_hash);
        CREATE INDEX IF NOT EXISTS idx_network_poll_log_link_started_at ON network_poll_log (link_id, started_at);
        CREATE INDEX IF NOT EXISTS idx_network_area_subscriptions_area_id ON network_area_subscriptions (area_id);
        CREATE INDEX IF NOT EXISTS idx_network_area_subscriptions_link_id ON network_area_subscriptions (link_id);
        CREATE INDEX IF NOT EXISTS idx_network_nodelist_network_id_zone_net_node_point ON network_nodelist (network_id, zone, net, node, point);
        CREATE INDEX IF NOT EXISTS idx_network_applications_status ON network_applications (status);
        CREATE INDEX IF NOT EXISTS idx_network_applications_assigned_address ON network_applications (assigned_address);
        CREATE INDEX IF NOT EXISTS idx_network_nodes_network_key_status ON network_nodes (network_key, status);
        CREATE INDEX IF NOT EXISTS idx_network_nodes_address ON network_nodes (address);
        CREATE INDEX IF NOT EXISTS idx_network_credentials_node_status ON network_credentials (node_id, status);
        CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions (user_id);
        CREATE INDEX IF NOT EXISTS idx_sessions_started_at ON sessions (started_at);
        CREATE INDEX IF NOT EXISTS idx_door_runs_door_id ON door_runs (door_id);
        CREATE INDEX IF NOT EXISTS idx_door_runs_user_id ON door_runs (user_id);
        CREATE INDEX IF NOT EXISTS idx_door_runs_started_at ON door_runs (started_at);

        CREATE TABLE IF NOT EXISTS file_areas (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            key TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(key)) > 0),
            name TEXT NOT NULL CHECK (LENGTH(TRIM(name)) > 0),
            description TEXT NOT NULL DEFAULT '',
            root_path TEXT NOT NULL CHECK (LENGTH(TRIM(root_path)) > 0),
            read_security_level INT NOT NULL DEFAULT 0 CHECK (read_security_level >= 0 AND read_security_level <= 255),
            download_security_level INT NOT NULL DEFAULT 10 CHECK (download_security_level >= 0 AND download_security_level <= 255),
            upload_security_level INT NOT NULL DEFAULT 0 CHECK (upload_security_level >= 0 AND upload_security_level <= 255),
            max_upload_bytes INT CHECK (max_upload_bytes IS NULL OR max_upload_bytes >= 0),
            enabled BOOL NOT NULL DEFAULT TRUE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS file_entries (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            area_id UUID NOT NULL REFERENCES file_areas(id) ON DELETE CASCADE,
            storage_name TEXT NOT NULL CHECK (LENGTH(TRIM(storage_name)) > 0),
            display_name TEXT NOT NULL CHECK (LENGTH(TRIM(display_name)) > 0),
            original_name TEXT,
            size_bytes INT NOT NULL DEFAULT 0 CHECK (size_bytes >= 0),
            content_crc32 TEXT,
            description TEXT NOT NULL DEFAULT '',
            uploader_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
            download_count INT NOT NULL DEFAULT 0 CHECK (download_count >= 0),
            approved BOOL NOT NULL DEFAULT FALSE,
            created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS file_transfers (
            id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
            node_number INT NOT NULL CHECK (node_number > 0),
            user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
            area_id UUID REFERENCES file_areas(id) ON DELETE SET NULL,
            file_entry_id UUID REFERENCES file_entries(id) ON DELETE SET NULL,
            direction TEXT NOT NULL CHECK (direction = 'download' OR direction = 'upload'),
            protocol TEXT NOT NULL CHECK (protocol = 'zmodem' OR protocol = 'xmodem_crc'),
            requested_name TEXT,
            storage_name TEXT,
            declared_size_bytes INT CHECK (declared_size_bytes IS NULL OR declared_size_bytes >= 0),
            transferred_payload_bytes INT NOT NULL DEFAULT 0 CHECK (transferred_payload_bytes >= 0),
            committed_size_bytes INT CHECK (committed_size_bytes IS NULL OR committed_size_bytes >= 0),
            started_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
            ended_at TIMESTAMPTZ,
            duration_ms INT CHECK (duration_ms IS NULL OR duration_ms >= 0),
            outcome TEXT NOT NULL DEFAULT 'started' CHECK (outcome = 'started' OR outcome = 'success' OR outcome = 'cancelled' OR outcome = 'failed'),
            error_code TEXT,
            error_message TEXT,
            retry_count INT NOT NULL DEFAULT 0 CHECK (retry_count >= 0)
        );

        CREATE INDEX IF NOT EXISTS idx_file_entries_area_id ON file_entries (area_id);
        CREATE INDEX IF NOT EXISTS idx_file_entries_approved ON file_entries (approved);
        CREATE INDEX IF NOT EXISTS idx_file_transfers_user_id ON file_transfers (user_id);
        CREATE INDEX IF NOT EXISTS idx_file_transfers_started_at ON file_transfers (started_at);",
    )?;
    set_schema_version(db, SCHEMA_VERSION)?;
    Ok(())
}

pub fn schema_version(db: &Db) -> decentdb::Result<i64> {
    let result = db.execute_with_params(
        "SELECT value FROM system_config WHERE key = $1",
        &[Value::Text("schema_version".to_string())],
    )?;
    let value = result
        .rows()
        .first()
        .and_then(|row| row.values().first())
        .ok_or_else(|| decentdb::DbError::sql("OxideBBS schema_version is missing"))?;

    parse_schema_version(value)
}

fn parse_schema_version(value: &Value) -> decentdb::Result<i64> {
    match value {
        Value::Text(raw) => raw.parse::<i64>().map_err(|error| {
            decentdb::DbError::sql(format!("invalid OxideBBS schema version {raw:?}: {error}"))
        }),
        other => Err(decentdb::DbError::sql(format!(
            "invalid OxideBBS schema_version value: {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use decentdb::DbConfig;

    fn init_schema_2_probe_db(db: &Db) {
        db.execute_batch(
            "CREATE TABLE system_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );

            CREATE TABLE users (
                id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
                alias TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(alias)) > 0),
                real_name TEXT NOT NULL CHECK (LENGTH(TRIM(real_name)) > 0),
                email TEXT,
                password_hash TEXT NOT NULL,
                security_level INT NOT NULL DEFAULT 10 CHECK (security_level >= 0 AND security_level <= 255),
                is_sysop BOOL NOT NULL DEFAULT FALSE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                last_login_at TIMESTAMPTZ,
                total_calls INT NOT NULL DEFAULT 0 CHECK (total_calls >= 0),
                time_bank_minutes INT NOT NULL DEFAULT 0 CHECK (time_bank_minutes >= 0),
                status TEXT NOT NULL DEFAULT 'active'
                    CHECK (status = 'active' OR status = 'locked' OR status = 'disabled')
            );

            CREATE TABLE message_areas (
                id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
                key TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(key)) > 0),
                name TEXT NOT NULL CHECK (LENGTH(TRIM(name)) > 0),
                description TEXT NOT NULL DEFAULT '',
                kind TEXT NOT NULL DEFAULT 'local',
                network_id TEXT,
                read_security_level INT NOT NULL DEFAULT 0 CHECK (read_security_level >= 0 AND read_security_level <= 255),
                post_security_level INT NOT NULL DEFAULT 10 CHECK (post_security_level >= 0 AND post_security_level <= 255),
                moderated BOOL NOT NULL DEFAULT FALSE,
                CHECK (kind = 'local' OR kind = 'echomail' OR kind = 'netmail')
            );

            CREATE TABLE messages (
                id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
                area_id UUID NOT NULL REFERENCES message_areas(id) ON DELETE CASCADE,
                author_user_id UUID NOT NULL REFERENCES users(id) ON DELETE RESTRICT,
                to_user_id UUID REFERENCES users(id) ON DELETE SET NULL,
                subject TEXT NOT NULL CHECK (LENGTH(TRIM(subject)) > 0),
                body TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
                reply_to_id UUID REFERENCES messages(id) ON DELETE SET NULL,
                network_message_id TEXT,
                visibility TEXT NOT NULL DEFAULT 'normal'
                    CHECK (visibility = 'normal' OR visibility = 'deleted' OR visibility = 'hidden')
            );

            CREATE INDEX IF NOT EXISTS idx_messages_area_created_at ON messages (area_id, created_at);
            CREATE INDEX IF NOT EXISTS idx_messages_author_user_id ON messages (author_user_id);
            CREATE INDEX IF NOT EXISTS idx_messages_to_user_id ON messages (to_user_id);",
        )
        .expect("create schema-2 probe tables");

        db.execute_with_params(
            "INSERT INTO system_config (key, value) VALUES ($1, $2)",
            &[
                Value::Text("schema_version".to_string()),
                Value::Text("2".to_string()),
            ],
        )
        .expect("seed schema version 2");
    }

    #[test]
    fn schema_initializes_to_current_version() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        init_schema(&db).expect("init schema");
        assert_eq!(schema_version(&db).expect("schema version"), SCHEMA_VERSION);

        let tables = [
            "system_config",
            "users",
            "auth_attempts",
            "audit_events",
            "message_areas",
            "messages",
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
            "sessions",
            "doors",
            "door_runs",
            "door_provider_credentials",
            "file_areas",
            "file_entries",
            "file_transfers",
        ];
        for table in &tables {
            let result = db
                .execute(&format!("SELECT * FROM {table} LIMIT 0"))
                .unwrap_or_else(|_| panic!("query {table}"));
            assert!(!result.columns().is_empty(), "{table} should exist");
        }
    }

    #[test]
    fn schema_init_is_idempotent() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        init_schema(&db).expect("first init");
        init_schema(&db).expect("second init");
        assert_eq!(
            schema_version(&db).expect("read schema version"),
            SCHEMA_VERSION
        );
    }

    #[test]
    fn schema_init_rejects_missing_schema_marker() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        db.execute_batch(
            "CREATE TABLE system_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .expect("create system_config");

        let err = init_schema(&db).expect_err("init should reject missing marker");
        assert!(err.to_string().contains("schema_version marker is missing"));
    }

    #[test]
    fn schema_init_rejects_tables_without_schema_marker() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        db.execute_batch(
            "CREATE TABLE users (
                id UUID PRIMARY KEY DEFAULT GEN_RANDOM_UUID(),
                alias TEXT NOT NULL UNIQUE CHECK (LENGTH(TRIM(alias)) > 0),
                real_name TEXT NOT NULL CHECK (LENGTH(TRIM(real_name)) > 0),
                password_hash TEXT NOT NULL
            );",
        )
        .expect("create user table");

        let err = init_schema(&db).expect_err("init should reject unmarked existing tables");
        assert!(err.to_string().contains("found existing database tables"));
    }

    #[test]
    fn schema_init_rejects_newer_schema_marker() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        db.execute_batch(
            "CREATE TABLE system_config (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            );",
        )
        .expect("create system_config");
        db.execute_with_params(
            "INSERT INTO system_config (key, value) VALUES ($1, $2)",
            &[
                Value::Text("schema_version".to_string()),
                Value::Text("999".to_string()),
            ],
        )
        .expect("seed future schema marker");

        let err = init_schema(&db).expect_err("init should reject future marker");
        assert!(err.to_string().contains("newer than supported version"));
    }

    #[test]
    fn schema_init_migrates_schema_2_to_current() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");

        init_schema_2_probe_db(&db);
        db.execute_with_params(
            "INSERT INTO message_areas (key, name, kind) VALUES ($1, $2, $3)",
            &[
                Value::Text("general".to_string()),
                Value::Text("General".to_string()),
                Value::Text("local".to_string()),
            ],
        )
        .expect("seed schema-2 area");
        db.execute_with_params(
            "INSERT INTO users (id, alias, real_name, password_hash)
             VALUES (UUID_PARSE('00000000-0000-4000-8000-000000000111'), 'alice', 'Alice User', 'hash')",
            &[],
        )
        .expect("seed schema-2 user");
        db.execute_with_params(
            "INSERT INTO messages (id, area_id, author_user_id, subject, body, created_at)
             VALUES (UUID_PARSE('00000000-0000-4000-8000-000000000211'), 
                (SELECT id FROM message_areas WHERE key = $1),
                UUID_PARSE('00000000-0000-4000-8000-000000000111'), 'Seed', 'Body', CURRENT_TIMESTAMP)",
            &[Value::Text("general".to_string())],
        )
        .expect("seed schema-2 message");

        init_schema(&db).expect("apply migrations");
        assert_eq!(schema_version(&db).expect("schema version"), SCHEMA_VERSION);

        let enabled = {
            let key = Value::Text("general".to_string());
            let result = db
                .execute_with_params("SELECT enabled FROM message_areas WHERE key = $1", &[key])
                .expect("enabled value");
            let row = result
                .rows()
                .first()
                .and_then(|row| row.values().first())
                .expect("enabled column");
            match row {
                Value::Bool(enabled) => *enabled,
                _ => false,
            }
        };
        assert!(enabled);

        let author_kind = {
            let result = db
                .execute_with_params(
                    "SELECT author_kind, author_display_name
                     FROM messages
                     WHERE id = UUID_PARSE($1)",
                    &[Value::Text(
                        "00000000-0000-4000-8000-000000000211".to_string(),
                    )],
                )
                .expect("query author metadata");
            let row = result
                .rows()
                .first()
                .and_then(|row| row.values().first())
                .expect("author metadata exists");
            match row {
                Value::Text(kind) => kind.clone(),
                _ => String::new(),
            }
        };
        assert_eq!(author_kind, "local");

        let network_tables = db
            .execute(
                "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'network_profiles'",
            )
            .expect("query network_profiles table");
        let table_count = network_tables
            .rows()
            .first()
            .and_then(|row| row.values().first())
            .unwrap_or(&Value::Int64(0));
        assert_ne!(table_count, &Value::Int64(0));
    }

    #[test]
    fn schema_uses_native_decentdb_types_and_foreign_keys() {
        let db = Db::open_or_create(":memory:", DbConfig::default()).expect("open DecentDB");
        init_schema(&db).expect("init schema");

        let columns = db
            .execute(
                "SELECT column_name, data_type
                 FROM information_schema.columns
                 WHERE table_name = 'sessions'
                 ORDER BY column_name",
            )
            .expect("columns");
        let pairs: Vec<(String, String)> = columns
            .rows()
            .iter()
            .map(|row| {
                let values = row.values();
                let name = match &values[0] {
                    Value::Text(value) => value.clone(),
                    other => panic!("unexpected column name value {other:?}"),
                };
                let data_type = match &values[1] {
                    Value::Text(value) => value.clone(),
                    other => panic!("unexpected data type value {other:?}"),
                };
                (name, data_type)
            })
            .collect();

        assert!(pairs.contains(&("id".to_string(), "UUID".to_string())));
        assert!(pairs.contains(&("remote_ip".to_string(), "IPADDR".to_string())));
        assert!(pairs.contains(&("started_at".to_string(), "TIMESTAMPTZ".to_string())));

        let foreign_keys = db
            .execute("SELECT * FROM pragma_foreign_key_list('messages')")
            .expect("foreign keys");
        assert!(
            foreign_keys.rows().len() >= 4,
            "messages should declare area/user/reply foreign keys"
        );
    }
}
