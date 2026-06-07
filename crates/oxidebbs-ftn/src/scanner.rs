use std::fs::{self, File};
use std::path::{Path, PathBuf};

use oxidebbs_db::{
    Db, MessageRecord, NetworkAreaRecord, NetworkLinkRecord, NetworkMessageRecord,
    NetworkPacketRecord, NetworkProfileRecord, Value, finish_network_packet,
    insert_network_message, insert_network_packet, list_messages_in_area, list_network_areas,
    list_network_links, list_network_messages, list_network_packets, list_network_subscriptions,
    update_network_packet_file_status,
};
use oxidebbs_network::FtnAddress;
use sha2::{Digest, Sha256};

use crate::{
    BundleCreator, FtnError, FtnPacket, MessageAttribute, PacketHeader, PacketMessage, PacketWriter,
};

/// Filesystem paths used by the FTN outbound scanner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannerPaths {
    /// Profile-scoped outbound spool root.
    pub outbound_root: PathBuf,
}

impl ScannerPaths {
    /// Build the default v1.2 runtime outbound layout for one network profile.
    #[must_use]
    pub fn under_runtime(runtime_root: impl AsRef<Path>, network_key: &str) -> Self {
        Self {
            outbound_root: runtime_root
                .as_ref()
                .join("network")
                .join(network_key)
                .join("outbound"),
        }
    }

    fn ready_dir(&self, link_key: &str) -> PathBuf {
        self.outbound_root.join(link_key).join("ready")
    }

    fn bundled_dir(&self, link_key: &str) -> PathBuf {
        self.outbound_root.join(link_key).join("bundled")
    }
}

/// Outbound FTN scan counters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanResult {
    pub links_scanned: usize,
    pub packets_created: usize,
    pub messages_scanned: usize,
    pub messages_skipped: usize,
    pub errors: Vec<String>,
}

/// DecentDB-backed FTN outbound scanner.
///
/// The scanner finds local messages in subscribed echomail areas, composes
/// Type-2+ packet files for enabled links, and records outbound packet/message
/// state in DecentDB. BinkP delivery is handled by the later poller phase.
pub struct Scanner<'db> {
    db: &'db Db,
    profile: NetworkProfileRecord,
    paths: ScannerPaths,
}

impl<'db> Scanner<'db> {
    /// Create a scanner for an already-open database and network profile.
    #[must_use]
    pub fn new(db: &'db Db, profile: NetworkProfileRecord, paths: ScannerPaths) -> Self {
        Self { db, profile, paths }
    }

    /// Scan all enabled links on the profile for outbound echomail.
    ///
    /// # Errors
    ///
    /// Returns I/O, packet, or database errors that prevent the scan from
    /// completing. Per-link empty scans are reported as skipped messages.
    pub fn scan(&self) -> Result<ScanResult, FtnError> {
        let mut result = ScanResult::default();
        let links = list_network_links(self.db)?
            .into_iter()
            .filter(|link| link.network_id == self.profile.id && link.enabled)
            .collect::<Vec<_>>();
        let areas = list_network_areas(self.db)?
            .into_iter()
            .filter(|area| area.network_id == self.profile.id && area.subscribed && !area.read_only)
            .collect::<Vec<_>>();
        let subscriptions = list_network_subscriptions(self.db)?;
        let exported = exported_message_keys(self.db)?;

        for link in links {
            result.links_scanned += 1;
            let mut packet_messages = Vec::new();
            let link_subscriptions = subscriptions
                .iter()
                .filter(|subscription| subscription.link_id == link.id && subscription.subscribed)
                .collect::<Vec<_>>();

            for area in &areas {
                if !link_subscriptions
                    .iter()
                    .any(|subscription| subscription.area_id == area.id)
                {
                    continue;
                }

                for message in list_messages_in_area(self.db, &area.local_area_id)? {
                    if !is_exportable_local_message(&message)
                        || exported.contains(&(link.id.clone(), message.id.clone()))
                    {
                        result.messages_skipped += 1;
                        continue;
                    }
                    packet_messages.push(scanned_message(area, &message));
                }
            }

            if packet_messages.is_empty() {
                continue;
            }

            let packet_path = self.write_packet_for_link(&link, &packet_messages)?;
            let packet_record = self.record_packet(&link, &packet_path)?;
            for scanned in packet_messages {
                self.record_network_message(&link, &packet_record.id, &scanned)?;
                result.messages_scanned += 1;
            }
            result.packets_created += 1;
        }

        Ok(result)
    }

    /// Rescan a specific area for a specific link.
    ///
    /// This method exports all messages from the specified area to the specified link,
    /// ignoring the "already exported" check. This is used for rescan requests where
    /// a link needs to receive all messages in an area again.
    ///
    /// # Errors
    ///
    /// Returns I/O, packet, or database errors that prevent the rescan from completing.
    pub fn rescan_for_link(
        &self,
        link: &NetworkLinkRecord,
        area_tag: &str,
    ) -> Result<ScanResult, FtnError> {
        let mut result = ScanResult::default();

        // Find the network area matching the area_tag
        let areas = list_network_areas(self.db)?;
        let area = areas
            .iter()
            .find(|a| a.network_id == self.profile.id && a.area_tag == area_tag)
            .ok_or_else(|| FtnError::Protocol(format!("area {} not found", area_tag)))?;

        if !area.subscribed || area.read_only {
            return Err(FtnError::Protocol(format!(
                "area {} is not subscribed or is read-only",
                area_tag
            )));
        }

        // Find all local messages in this area
        let messages = list_messages_in_area(self.db, &area.local_area_id)?;
        let mut packet_messages = Vec::new();

        for message in messages {
            if !is_exportable_local_message(&message) {
                result.messages_skipped += 1;
                continue;
            }
            packet_messages.push(scanned_message(area, &message));
        }

        if packet_messages.is_empty() {
            return Ok(result);
        }

        // Create packet for this specific link
        result.links_scanned = 1;
        let packet_path = self.write_packet_for_link(link, &packet_messages)?;
        let packet_record = self.record_packet(link, &packet_path)?;

        for scanned in packet_messages {
            self.record_network_message(link, &packet_record.id, &scanned)?;
            result.messages_scanned += 1;
        }
        result.packets_created = 1;

        Ok(result)
    }

    /// Bundle ready packets for a link into a ZIP archive with FTN-standard naming.
    ///
    /// This method finds all `.pkt` files in the ready directory for the specified
    /// link, creates a ZIP bundle with a name based on the link's net/node address,
    /// and moves the bundled packets to a `bundled` directory.
    ///
    /// Returns the path to the created bundle, or `None` if no packets were found.
    ///
    /// # Errors
    ///
    /// Returns I/O or bundle creation errors if the operation fails.
    pub fn bundle_ready_packets(
        &self,
        link: &NetworkLinkRecord,
    ) -> Result<Option<PathBuf>, FtnError> {
        let ready_dir = self.paths.ready_dir(&link.key);
        if !ready_dir.exists() {
            return Ok(None);
        }

        let mut packet_paths: Vec<PathBuf> = fs::read_dir(&ready_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "pkt")
                    .unwrap_or(false)
            })
            .map(|entry| entry.path())
            .collect();
        packet_paths.sort();

        if packet_paths.is_empty() {
            return Ok(None);
        }

        let bundled_dir = self.paths.bundled_dir(&link.key);
        fs::create_dir_all(&bundled_dir)?;

        let bundle_name = format!(
            "{:04x}{:04x}.zip",
            link.address
                .parse::<FtnAddress>()
                .map(|addr| addr.net)
                .unwrap_or(0),
            link.address
                .parse::<FtnAddress>()
                .map(|addr| addr.node)
                .unwrap_or(0)
        );
        let bundle_path = available_path(&bundled_dir.join(&bundle_name));

        BundleCreator::create_zip_bundle(&packet_paths, &bundle_path)?;
        let now = current_timestamp(self.db)?;
        let bundle_packet = NetworkPacketRecord {
            id: generated_uuid(self.db)?,
            network_id: self.profile.id.clone(),
            direction: "outbound".to_string(),
            link_id: Some(link.id.clone()),
            filename: bundle_path.display().to_string(),
            sha256: sha256_file(&bundle_path)?,
            size_bytes: fs::metadata(&bundle_path)?
                .len()
                .try_into()
                .unwrap_or(i64::MAX),
            status: "pending".to_string(),
            error_message: None,
            received_at: None,
            processed_at: None,
            created_at: now,
        };
        insert_network_packet(self.db, &bundle_packet)?;

        let packet_rows = list_network_packets(self.db)?;
        for packet_path in &packet_paths {
            let packet_name = packet_path.display().to_string();
            for packet in packet_rows.iter().filter(|packet| {
                packet.direction == "outbound"
                    && packet.link_id.as_deref() == Some(link.id.as_str())
                    && packet.filename == packet_name
                    && packet.status == "pending"
            }) {
                finish_network_packet(
                    self.db,
                    &packet.id,
                    "processed",
                    Some("bundled into outbound ZIP arcmail"),
                )?;
            }
        }

        for packet_path in &packet_paths {
            fs::remove_file(packet_path)?;
        }

        Ok(Some(bundle_path))
    }

    /// Materialize pending outbound netmail packets into .pkt files.
    ///
    /// This method finds pending outbound netmail (AreaFix replies and forwarded
    /// netmail) in the database, groups them by link, and writes .pkt files to
    /// the ready directory for BinkP delivery.
    ///
    /// Returns the number of packets materialized.
    ///
    /// # Errors
    ///
    /// Returns I/O, packet, or database errors that prevent materialization.
    pub fn materialize_outbound_netmail(&self) -> Result<usize, FtnError> {
        let packets = list_network_packets(self.db)?
            .into_iter()
            .filter(|p| {
                p.network_id == self.profile.id
                    && p.direction == "outbound"
                    && p.status == "pending"
                    && p.link_id.is_some()
            })
            .collect::<Vec<_>>();

        if packets.is_empty() {
            return Ok(0);
        }

        let all_messages = list_network_messages(self.db)?;
        let links = list_network_links(self.db)?;
        let mut materialized = 0;

        for packet in packets {
            if Path::new(&packet.filename).exists() {
                continue;
            }

            let link_id = match packet.link_id.as_ref() {
                Some(id) => id,
                None => continue,
            };

            let link = match links.iter().find(|l| l.id == *link_id) {
                Some(l) => l,
                None => continue,
            };

            let netmail_messages: Vec<PacketMessage> = all_messages
                .iter()
                .filter(|m| {
                    m.packet_id.as_deref() == Some(&packet.id)
                        && m.message_type == "netmail"
                        && m.status == "pending"
                })
                .map(|m| PacketMessage {
                    to_user: m.to_name.clone().unwrap_or_else(|| "Sysop".to_string()),
                    from_user: m.from_name.clone(),
                    subject: m.subject.clone(),
                    body: m.raw_text.clone(),
                    area_tag: String::new(),
                    attributes: MessageAttribute::PRIVATE,
                })
                .collect();

            if netmail_messages.is_empty() {
                continue;
            }

            let ready_dir = self.paths.ready_dir(&link.key);
            fs::create_dir_all(&ready_dir)?;
            let packet_path = available_path(&ready_dir.join(packet_file_name(&packet)?));

            let ftn_packet = FtnPacket {
                header: packet_header(&self.profile, link)?,
                messages: netmail_messages,
            };

            let mut file = File::options()
                .write(true)
                .create_new(true)
                .open(&packet_path)?;
            PacketWriter::write(&mut file, &ftn_packet)?;

            update_network_packet_file_status(
                self.db,
                &packet.id,
                &packet_path.display().to_string(),
                &sha256_file(&packet_path)?,
                fs::metadata(&packet_path)?
                    .len()
                    .try_into()
                    .unwrap_or(i64::MAX),
                "pending",
            )?;
            materialized += 1;
        }

        Ok(materialized)
    }

    fn write_packet_for_link(
        &self,
        link: &NetworkLinkRecord,
        messages: &[ScannedMessage],
    ) -> Result<PathBuf, FtnError> {
        let ready_dir = self.paths.ready_dir(&link.key);
        fs::create_dir_all(&ready_dir)?;
        let packet_path =
            ready_dir.join(format!("{}.pkt", generated_uuid(self.db)?.replace('-', "")));
        let packet = FtnPacket {
            header: packet_header(&self.profile, link)?,
            messages: messages
                .iter()
                .map(|message| message.packet_message.clone())
                .collect(),
        };
        let mut file = File::options()
            .write(true)
            .create_new(true)
            .open(&packet_path)?;
        PacketWriter::write(&mut file, &packet)?;
        Ok(packet_path)
    }

    fn record_packet(
        &self,
        link: &NetworkLinkRecord,
        packet_path: &Path,
    ) -> Result<NetworkPacketRecord, FtnError> {
        let now = current_timestamp(self.db)?;
        let packet = NetworkPacketRecord {
            id: generated_uuid(self.db)?,
            network_id: self.profile.id.clone(),
            direction: "outbound".to_string(),
            link_id: Some(link.id.clone()),
            filename: packet_path.display().to_string(),
            sha256: sha256_file(packet_path)?,
            size_bytes: fs::metadata(packet_path)?
                .len()
                .try_into()
                .unwrap_or(i64::MAX),
            status: "pending".to_string(),
            error_message: None,
            received_at: None,
            processed_at: None,
            created_at: now,
        };
        insert_network_packet(self.db, &packet)?;
        Ok(packet)
    }

    fn record_network_message(
        &self,
        link: &NetworkLinkRecord,
        packet_id: &str,
        scanned: &ScannedMessage,
    ) -> Result<(), FtnError> {
        let now = current_timestamp(self.db)?;
        insert_network_message(
            self.db,
            &NetworkMessageRecord {
                id: generated_uuid(self.db)?,
                network_id: self.profile.id.clone(),
                local_message_id: Some(scanned.local_message.id.clone()),
                message_type: "echomail".to_string(),
                area_tag: Some(scanned.area.area_tag.clone()),
                origin_address: profile_address(&self.profile)?.to_string(),
                destination_address: Some(link.address.clone()),
                from_name: scanned.local_message.author_display_name.clone(),
                to_name: Some(nonblank(&scanned.packet_message.to_user, "All")),
                subject: scanned.local_message.subject.clone(),
                raw_text: scanned.packet_message.body.clone(),
                display_body: scanned.local_message.body.clone(),
                msgid: Some(scanned.msgid.clone()),
                replyid: scanned.local_message.reply_to_id.clone(),
                created_at: scanned.local_message.created_at.clone(),
                imported_at: None,
                exported_at: Some(now),
                duplicate_hash: None,
                packet_id: Some(packet_id.to_string()),
                status: "exported".to_string(),
            },
        )?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct ScannedMessage {
    area: NetworkAreaRecord,
    local_message: MessageRecord,
    packet_message: PacketMessage,
    msgid: String,
}

fn scanned_message(area: &NetworkAreaRecord, message: &MessageRecord) -> ScannedMessage {
    let msgid = format!("{} {}", area.area_tag, message.id);
    let body = format!(
        "\x01MSGID: {msgid}\r{}\rSEEN-BY: \rPATH: \r * Origin: OxideBBS",
        normalize_body(&message.body)
    );
    ScannedMessage {
        area: area.clone(),
        local_message: message.clone(),
        packet_message: PacketMessage {
            to_user: "All".to_string(),
            from_user: nonblank(&message.author_display_name, "Sysop"),
            subject: nonblank(&message.subject, "(no subject)"),
            body: body.into_bytes(),
            area_tag: area.area_tag.clone(),
            attributes: MessageAttribute::NONE,
        },
        msgid,
    }
}

fn exported_message_keys(db: &Db) -> Result<Vec<(String, String)>, FtnError> {
    let packets = list_network_packets(db)?;
    let messages = list_network_messages(db)?;
    Ok(messages
        .into_iter()
        .filter(|message| message.status == "exported")
        .filter_map(|message| {
            let local_message_id = message.local_message_id?;
            let packet_id = message.packet_id?;
            let link_id = packets
                .iter()
                .find(|packet| packet.id == packet_id)
                .and_then(|packet| packet.link_id.clone())?;
            Some((link_id, local_message_id))
        })
        .collect())
}

fn is_exportable_local_message(message: &MessageRecord) -> bool {
    message.visibility == "normal" && matches!(message.author_kind.as_str(), "local" | "system")
}

fn packet_header(
    profile: &NetworkProfileRecord,
    link: &NetworkLinkRecord,
) -> Result<PacketHeader, FtnError> {
    let origin = profile_address(profile)?;
    let destination = link
        .address
        .parse::<FtnAddress>()
        .map_err(|error| FtnError::Protocol(error.to_string()))?;
    let mut header = PacketHeader {
        orig_node: origin.node,
        orig_net: origin.net,
        orig_zone: origin.zone,
        dest_node: destination.node,
        dest_net: destination.net,
        dest_zone: destination.zone,
        orig_net2: origin.net,
        dest_net2: destination.net,
        orig_zone2: origin.zone,
        dest_zone2: destination.zone,
        ..PacketHeader::default()
    };
    for (index, byte) in link.password.as_bytes().iter().take(8).enumerate() {
        header.password[index] = *byte;
    }
    Ok(header)
}

fn profile_address(profile: &NetworkProfileRecord) -> Result<FtnAddress, FtnError> {
    Ok(FtnAddress {
        zone: u16::try_from(profile.local_zone)
            .map_err(|_| FtnError::Protocol("profile local zone is out of range".to_string()))?,
        net: u16::try_from(profile.local_net)
            .map_err(|_| FtnError::Protocol("profile local net is out of range".to_string()))?,
        node: u16::try_from(profile.local_node)
            .map_err(|_| FtnError::Protocol("profile local node is out of range".to_string()))?,
        point: (profile.local_point > 0)
            .then(|| u16::try_from(profile.local_point))
            .transpose()
            .map_err(|_| FtnError::Protocol("profile local point is out of range".to_string()))?,
    })
}

fn current_timestamp(db: &Db) -> Result<String, FtnError> {
    scalar_text(db, "SELECT CAST(NOW() AS TEXT)")
}

fn generated_uuid(db: &Db) -> Result<String, FtnError> {
    scalar_text(db, "SELECT UUID_TO_STRING(GEN_RANDOM_UUID())")
}

fn scalar_text(db: &Db, sql: &str) -> Result<String, FtnError> {
    let result = db.execute(sql)?;
    match result.rows().first().and_then(|row| row.values().first()) {
        Some(Value::Text(value)) => Ok(value.clone()),
        Some(other) => Err(FtnError::Protocol(format!(
            "query returned non-text scalar for {sql}: {other:?}"
        ))),
        None => Err(FtnError::Protocol(format!(
            "query returned no scalar value: {sql}"
        ))),
    }
}

fn nonblank(value: &str, fallback: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_body(body: &str) -> String {
    body.replace('\n', "\r")
}

fn packet_file_name(packet: &NetworkPacketRecord) -> Result<String, FtnError> {
    let path = Path::new(&packet.filename);
    let candidate = path
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("netmail.pkt");
    let extension = Path::new(candidate)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("pkt") {
        Ok(candidate.to_string())
    } else {
        Ok(format!("{}.pkt", generated_uuid_from_text(&packet.id)))
    }
}

fn generated_uuid_from_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| *character != '-')
        .collect()
}

fn sha256_file(path: &Path) -> Result<String, FtnError> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(hex_nibble(byte >> 4));
        out.push(hex_nibble(byte & 0x0F));
    }
    out
}

fn hex_nibble(nibble: u8) -> char {
    match nibble {
        0..=9 => char::from(b'0' + nibble),
        _ => char::from(b'a' + (nibble - 10)),
    }
}

fn available_path(destination: &Path) -> PathBuf {
    if !destination.exists() {
        return destination.to_path_buf();
    }
    let parent = destination.parent().unwrap_or_else(|| Path::new(""));
    let stem = destination
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("packet");
    let extension = destination
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| format!(".{extension}"))
        .unwrap_or_default();
    for index in 1.. {
        let candidate = parent.join(format!("{stem}.{index}{extension}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    destination.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{BundleExtractor, PacketReader, Tosser, TosserPaths};
    use oxidebbs_db::{
        DbWriter, MessageAreaRecord, NetworkSubscriptionRecord, OxideDb, insert_message,
        insert_message_area, insert_network_area, insert_network_link, insert_network_profile,
        insert_network_subscription, list_messages, list_network_messages, list_network_packets,
    };
    use std::sync::{Arc, Barrier};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    const PROFILE_ID: &str = "00000000-0000-4000-8000-000000002001";
    const LINK_ID: &str = "00000000-0000-4000-8000-000000002002";
    const AREA_ID: &str = "00000000-0000-4000-8000-000000002003";
    const NETWORK_AREA_ID: &str = "00000000-0000-4000-8000-000000002004";
    const SUB_ID: &str = "00000000-0000-4000-8000-000000002005";
    const USER_ID: &str = "00000000-0000-4000-8000-000000002006";
    const MESSAGE_ID: &str = "00000000-0000-4000-8000-000000002007";

    fn test_db() -> OxideDb {
        let db = OxideDb::open_memory().expect("open db");
        seed_db(&db);
        db
    }

    fn seed_db(db: &OxideDb) {
        oxidebbs_db::insert_user(
            db.db(),
            &oxidebbs_db::UserRecord {
                id: USER_ID.to_string(),
                alias: "sysop".to_string(),
                real_name: "Sysop".to_string(),
                email: None,
                password_hash: "hash".to_string(),
                security_level: 255,
                is_sysop: true,
                created_at: "2026-06-04T00:00:00Z".to_string(),
                last_login_at: None,
                total_calls: 0,
                time_bank_minutes: 0,
                status: "active".to_string(),
            },
        )
        .expect("insert user");
        insert_message_area(
            db.db(),
            &MessageAreaRecord {
                id: AREA_ID.to_string(),
                key: "oxide.general".to_string(),
                name: "Oxide General".to_string(),
                description: "Network general".to_string(),
                kind: "echomail".to_string(),
                network_id: Some(PROFILE_ID.to_string()),
                read_security_level: 0,
                post_security_level: 10,
                moderated: false,
                enabled: true,
            },
        )
        .expect("insert area");
        insert_network_profile(db.db(), &profile()).expect("insert profile");
        insert_network_link(db.db(), &link()).expect("insert link");
        insert_network_area(
            db.db(),
            &NetworkAreaRecord {
                id: NETWORK_AREA_ID.to_string(),
                network_id: PROFILE_ID.to_string(),
                area_tag: "OXIDE.GENERAL".to_string(),
                local_area_id: AREA_ID.to_string(),
                description: "General".to_string(),
                read_only: false,
                subscribed: true,
                created_at: "2026-06-04T00:00:00Z".to_string(),
                updated_at: "2026-06-04T00:00:00Z".to_string(),
            },
        )
        .expect("insert network area");
        insert_network_subscription(
            db.db(),
            &NetworkSubscriptionRecord {
                id: SUB_ID.to_string(),
                area_id: NETWORK_AREA_ID.to_string(),
                link_id: LINK_ID.to_string(),
                subscribed: true,
                subscribed_at: "2026-06-04T00:00:00Z".to_string(),
                unsubscribed_at: None,
                source: "manual".to_string(),
            },
        )
        .expect("insert subscription");
    }

    fn profile() -> NetworkProfileRecord {
        NetworkProfileRecord {
            id: PROFILE_ID.to_string(),
            key: "fidonet".to_string(),
            name: "FidoNet".to_string(),
            adapter: "legacy-ftn".to_string(),
            local_zone: 1,
            local_net: 105,
            local_node: 42,
            local_point: 0,
            enabled: true,
            created_at: "2026-06-04T00:00:00Z".to_string(),
            updated_at: "2026-06-04T00:00:00Z".to_string(),
        }
    }

    fn ftn_password() -> String {
        format!("{:08}", std::process::id() % 100_000_000)
    }

    fn link() -> NetworkLinkRecord {
        NetworkLinkRecord {
            id: LINK_ID.to_string(),
            key: "hub".to_string(),
            network_id: PROFILE_ID.to_string(),
            address: "1:105/1".to_string(),
            host: "hub.example".to_string(),
            binkp_port: 24554,
            password: ftn_password(),
            poll_schedule_minutes: 60,
            compression: "none".to_string(),
            transport_security: "plaintext_legacy".to_string(),
            enabled: true,
            created_at: "2026-06-04T00:00:00Z".to_string(),
            updated_at: "2026-06-04T00:00:00Z".to_string(),
        }
    }

    fn insert_local_message(db: &OxideDb) {
        insert_message(
            db.db(),
            &MessageRecord {
                id: MESSAGE_ID.to_string(),
                area_id: AREA_ID.to_string(),
                author_user_id: USER_ID.to_string(),
                author_kind: "local".to_string(),
                author_display_name: "sysop".to_string(),
                author_network_address: None,
                to_user_id: None,
                subject: "Local hello".to_string(),
                body: "Outbound body".to_string(),
                created_at: "2026-06-04T00:00:00Z".to_string(),
                reply_to_id: None,
                network_message_id: None,
                visibility: "normal".to_string(),
            },
        )
        .expect("insert message");
    }

    fn temp_root(test_name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("oxidebbs-scanner-{test_name}-{suffix}"))
    }

    fn write_inbound_packet(root: &Path, name: &str, msgid_suffix: &str) -> PathBuf {
        let path = root
            .join("network")
            .join("fidonet")
            .join("inbound")
            .join("drop")
            .join(name);
        fs::create_dir_all(path.parent().expect("packet parent")).expect("create inbound");
        let body = format!(
            "AREA:OXIDE.GENERAL\r\x01MSGID: 1:105/1 {msgid_suffix}\rInbound body\rSEEN-BY: 105/1\rPATH: 105/1\r"
        );
        let password = ftn_password();
        let packet = FtnPacket {
            header: inbound_header(&password),
            messages: vec![PacketMessage {
                to_user: "All".to_string(),
                from_user: "Hub".to_string(),
                subject: "Inbound hello".to_string(),
                body: body.into_bytes(),
                area_tag: "OXIDE.GENERAL".to_string(),
                attributes: MessageAttribute::NONE,
            }],
        };
        let mut bytes = Vec::new();
        PacketWriter::write(&mut bytes, &packet).expect("write packet");
        fs::write(&path, bytes).expect("write packet file");
        path
    }

    fn inbound_header(password: &str) -> PacketHeader {
        let mut header = PacketHeader {
            orig_node: 1,
            orig_net: 105,
            orig_zone: 1,
            dest_node: 42,
            dest_net: 105,
            dest_zone: 1,
            orig_net2: 105,
            dest_net2: 105,
            orig_zone2: 1,
            dest_zone2: 1,
            ..PacketHeader::default()
        };
        for (index, byte) in password.as_bytes().iter().take(8).enumerate() {
            header.password[index] = *byte;
        }
        header
    }

    #[test]
    fn scans_local_echomail_into_outbound_packet() {
        let db = test_db();
        insert_local_message(&db);
        let root = temp_root("outbound");
        let scanner = Scanner::new(
            db.db(),
            profile(),
            ScannerPaths::under_runtime(&root, "fidonet"),
        );

        let result = scanner.scan().expect("scan");

        assert_eq!(result.links_scanned, 1);
        assert_eq!(result.packets_created, 1);
        assert_eq!(result.messages_scanned, 1);
        let ready_dir = root.join("network/fidonet/outbound/hub/ready");
        let packet_path = fs::read_dir(&ready_dir)
            .expect("ready dir")
            .next()
            .expect("packet")
            .expect("packet entry")
            .path();
        let packet =
            PacketReader::read(File::open(packet_path).expect("open packet")).expect("read packet");
        assert_eq!(packet.header.orig_node, 42);
        assert_eq!(packet.header.dest_node, 1);
        assert_eq!(packet.messages[0].area_tag, "OXIDE.GENERAL");
        assert!(String::from_utf8_lossy(&packet.messages[0].body).contains("Outbound body"));
        assert_eq!(
            list_network_messages(db.db())
                .expect("network messages")
                .len(),
            1
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn scan_does_not_export_same_message_twice_to_same_link() {
        let db = test_db();
        insert_local_message(&db);
        let root = temp_root("dedupe");
        let scanner = Scanner::new(
            db.db(),
            profile(),
            ScannerPaths::under_runtime(&root, "fidonet"),
        );

        let first = scanner.scan().expect("first scan");
        let second = scanner.scan().expect("second scan");

        assert_eq!(first.messages_scanned, 1);
        assert_eq!(second.messages_scanned, 0);
        assert_eq!(
            list_network_messages(db.db())
                .expect("network messages")
                .len(),
            1
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn bundle_ready_packets_creates_pending_zip_bundle_row() {
        let db = test_db();
        insert_local_message(&db);
        let root = temp_root("bundle");
        let scanner = Scanner::new(
            db.db(),
            profile(),
            ScannerPaths::under_runtime(&root, "fidonet"),
        );
        scanner.scan().expect("scan");

        let bundle = scanner
            .bundle_ready_packets(&link())
            .expect("bundle ready packets")
            .expect("bundle created");

        assert_eq!(bundle.extension().and_then(|ext| ext.to_str()), Some("zip"));
        assert!(bundle.exists());
        let output_dir = root.join("extracted");
        let extracted =
            BundleExtractor::extract_packets(&bundle, &output_dir).expect("extract bundle");
        assert_eq!(extracted.len(), 1);

        let packets = list_network_packets(db.db()).expect("packets");
        assert!(packets.iter().any(|packet| {
            packet.filename == bundle.display().to_string()
                && packet.direction == "outbound"
                && packet.status == "pending"
        }));
        assert!(packets.iter().any(|packet| {
            packet.filename.ends_with(".pkt")
                && packet.direction == "outbound"
                && packet.status == "processed"
        }));
        let ready_dir = root.join("network/fidonet/outbound/hub/ready");
        let remaining_ready_packets = fs::read_dir(&ready_dir)
            .expect("ready dir")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("pkt"))
            .count();
        assert_eq!(remaining_ready_packets, 0);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn materialize_outbound_netmail_creates_pkt_file() {
        use oxidebbs_db::{
            NetworkMessageRecord, NetworkPacketRecord, insert_network_message,
            insert_network_packet, list_network_packets,
        };

        let db = test_db();
        let root = temp_root("netmail");

        let packet_id = "00000000-0000-4000-8000-000000003001";
        let message_id = "00000000-0000-4000-8000-000000003002";

        insert_network_packet(
            db.db(),
            &NetworkPacketRecord {
                id: packet_id.to_string(),
                network_id: PROFILE_ID.to_string(),
                direction: "outbound".to_string(),
                link_id: Some(LINK_ID.to_string()),
                filename: "areafix-reply.pkt".to_string(),
                sha256: "abc123".to_string(),
                size_bytes: 100,
                status: "pending".to_string(),
                error_message: None,
                received_at: None,
                processed_at: None,
                created_at: "2026-06-04T00:00:00Z".to_string(),
            },
        )
        .expect("insert packet");

        insert_network_message(
            db.db(),
            &NetworkMessageRecord {
                id: message_id.to_string(),
                network_id: PROFILE_ID.to_string(),
                local_message_id: None,
                message_type: "netmail".to_string(),
                area_tag: None,
                origin_address: "1:105/42".to_string(),
                destination_address: Some("1:105/1".to_string()),
                from_name: "AreaFix".to_string(),
                to_name: Some("Sysop".to_string()),
                subject: "AreaFix Response".to_string(),
                raw_text: b"AreaFix reply body".to_vec(),
                display_body: "AreaFix reply body".to_string(),
                msgid: None,
                replyid: None,
                created_at: "2026-06-04T00:00:00Z".to_string(),
                imported_at: None,
                exported_at: None,
                duplicate_hash: None,
                packet_id: Some(packet_id.to_string()),
                status: "pending".to_string(),
            },
        )
        .expect("insert message");

        let scanner = Scanner::new(
            db.db(),
            profile(),
            ScannerPaths::under_runtime(&root, "fidonet"),
        );

        let materialized = scanner.materialize_outbound_netmail().expect("materialize");
        assert_eq!(materialized, 1);

        let ready_dir = root.join("network/fidonet/outbound/hub/ready");
        assert!(ready_dir.exists());
        let pkt_files: Vec<_> = fs::read_dir(&ready_dir)
            .expect("ready dir")
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("pkt"))
            .collect();
        assert_eq!(pkt_files.len(), 1);

        let packet = PacketReader::read(File::open(pkt_files[0].path()).expect("open"))
            .expect("read packet");
        assert_eq!(packet.header.orig_node, 42);
        assert_eq!(packet.header.dest_node, 1);
        assert_eq!(packet.messages.len(), 1);
        assert_eq!(packet.messages[0].from_user, "AreaFix");
        assert_eq!(packet.messages[0].to_user, "Sysop");
        assert_eq!(packet.messages[0].subject, "AreaFix Response");
        assert!(packet.messages[0].area_tag.is_empty());

        let updated_packets = list_network_packets(db.db()).expect("packets");
        let updated = updated_packets
            .iter()
            .find(|p| p.id == packet_id)
            .expect("packet");
        assert_eq!(updated.status, "pending");
        assert!(Path::new(&updated.filename).exists());
        assert_eq!(
            updated.size_bytes,
            fs::metadata(&updated.filename).expect("metadata").len() as i64
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn materialize_outbound_netmail_is_scoped_to_scanner_profile() {
        use oxidebbs_db::{
            NetworkMessageRecord, NetworkPacketRecord, insert_network_message,
            insert_network_packet, list_network_packets,
        };

        let db = test_db();
        let root = temp_root("netmail-profile-scope");
        let other_profile = NetworkProfileRecord {
            id: "00000000-0000-4000-8000-000000003101".to_string(),
            key: "othernet".to_string(),
            name: "OtherNet".to_string(),
            ..profile()
        };
        let other_link = NetworkLinkRecord {
            id: "00000000-0000-4000-8000-000000003102".to_string(),
            key: "otherhub".to_string(),
            network_id: other_profile.id.clone(),
            ..link()
        };
        insert_network_profile(db.db(), &other_profile).expect("insert other profile");
        insert_network_link(db.db(), &other_link).expect("insert other link");

        for (packet_id, message_id, network_id, link_id) in [
            (
                "00000000-0000-4000-8000-000000003103",
                "00000000-0000-4000-8000-000000003104",
                PROFILE_ID,
                LINK_ID,
            ),
            (
                "00000000-0000-4000-8000-000000003105",
                "00000000-0000-4000-8000-000000003106",
                other_profile.id.as_str(),
                other_link.id.as_str(),
            ),
        ] {
            insert_network_packet(
                db.db(),
                &NetworkPacketRecord {
                    id: packet_id.to_string(),
                    network_id: network_id.to_string(),
                    direction: "outbound".to_string(),
                    link_id: Some(link_id.to_string()),
                    filename: format!("{packet_id}.pkt"),
                    sha256: "abc123".to_string(),
                    size_bytes: 100,
                    status: "pending".to_string(),
                    error_message: None,
                    received_at: None,
                    processed_at: None,
                    created_at: "2026-06-04T00:00:00Z".to_string(),
                },
            )
            .expect("insert packet");
            insert_network_message(
                db.db(),
                &NetworkMessageRecord {
                    id: message_id.to_string(),
                    network_id: network_id.to_string(),
                    local_message_id: None,
                    message_type: "netmail".to_string(),
                    area_tag: None,
                    origin_address: "1:105/42".to_string(),
                    destination_address: Some("1:105/1".to_string()),
                    from_name: "AreaFix".to_string(),
                    to_name: Some("Sysop".to_string()),
                    subject: "AreaFix Response".to_string(),
                    raw_text: b"AreaFix reply body".to_vec(),
                    display_body: "AreaFix reply body".to_string(),
                    msgid: None,
                    replyid: None,
                    created_at: "2026-06-04T00:00:00Z".to_string(),
                    imported_at: None,
                    exported_at: None,
                    duplicate_hash: None,
                    packet_id: Some(packet_id.to_string()),
                    status: "pending".to_string(),
                },
            )
            .expect("insert message");
        }

        let scanner = Scanner::new(
            db.db(),
            profile(),
            ScannerPaths::under_runtime(&root, "fidonet"),
        );

        let materialized = scanner.materialize_outbound_netmail().expect("materialize");

        assert_eq!(materialized, 1);
        let packets = list_network_packets(db.db()).expect("packets");
        let local_packet = packets
            .iter()
            .find(|packet| packet.network_id == PROFILE_ID)
            .expect("local packet");
        let other_packet = packets
            .iter()
            .find(|packet| packet.network_id == other_profile.id)
            .expect("other packet");
        assert!(Path::new(&local_packet.filename).exists());
        assert_eq!(
            other_packet.filename,
            "00000000-0000-4000-8000-000000003105.pkt"
        );
        let other_ready_dir = root.join("network/fidonet/outbound/otherhub/ready");
        assert!(!other_ready_dir.exists());

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn concurrent_scan_and_toss_complete_through_db_writer() {
        let root = temp_root("concurrent");
        fs::create_dir_all(&root).expect("create root");
        let db = test_db();
        insert_local_message(&db);
        write_inbound_packet(&root, "concurrent-in.pkt", "concurrent-in");

        let writer = Arc::new(DbWriter::new(db.db().clone()));
        let barrier = Arc::new(Barrier::new(3));
        let scanner_barrier = Arc::clone(&barrier);
        let scanner_writer = Arc::clone(&writer);
        let scanner_root = root.clone();
        let scanner_thread = thread::spawn(move || {
            scanner_barrier.wait();
            let ticket = scanner_writer
                .submit(move |db| {
                    let scanner = Scanner::new(
                        db,
                        profile(),
                        ScannerPaths::under_runtime(&scanner_root, "fidonet"),
                    );
                    Ok(match scanner.scan() {
                        Ok(result) => Ok(result),
                        Err(FtnError::Database(error)) => return Err(error),
                        Err(error) => Err(error.to_string()),
                    })
                })
                .expect("submit scan");
            ticket.wait().expect("scan writer").expect("scan")
        });

        let tosser_barrier = Arc::clone(&barrier);
        let tosser_writer = Arc::clone(&writer);
        let tosser_root = root.clone();
        let tosser_thread = thread::spawn(move || {
            tosser_barrier.wait();
            let ticket = tosser_writer
                .submit(move |db| {
                    let tosser = Tosser::new(
                        db,
                        profile(),
                        TosserPaths::under_runtime(&tosser_root, "fidonet"),
                    );
                    Ok(match tosser.toss() {
                        Ok(result) => Ok(result),
                        Err(FtnError::Database(error)) => return Err(error),
                        Err(error) => Err(error.to_string()),
                    })
                })
                .expect("submit toss");
            ticket.wait().expect("toss writer").expect("toss")
        });

        barrier.wait();
        let scan_result = scanner_thread.join().expect("scanner thread");
        let toss_result = tosser_thread.join().expect("tosser thread");

        assert_eq!(scan_result.messages_scanned, 1);
        assert_eq!(scan_result.packets_created, 1);
        assert_eq!(toss_result.messages_imported, 1);
        assert_eq!(toss_result.packets_processed, 1);

        let local_messages = list_messages(db.db()).expect("local messages");
        assert_eq!(local_messages.len(), 2);
        assert!(
            local_messages
                .iter()
                .any(|message| message.author_kind == "network")
        );
        let network_messages = list_network_messages(db.db()).expect("network messages");
        assert!(
            network_messages
                .iter()
                .any(|message| message.status == "imported")
        );
        assert!(
            network_messages
                .iter()
                .any(|message| message.status == "exported")
        );

        let _ = fs::remove_dir_all(root);
    }
}
