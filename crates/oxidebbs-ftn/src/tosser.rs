use std::fs::{self, File};
use std::path::{Path, PathBuf};

use oxidebbs_db::{
    Db, MessageRecord, NetworkDuplicateLogRecord, NetworkLinkRecord, NetworkMessageRecord,
    NetworkPacketRecord, NetworkPathNode, NetworkProfileRecord, NetworkSeenByNode, Value,
    find_network_area_by_tag_and_profile, finish_network_packet, insert_message,
    insert_network_duplicate_log, insert_network_message, insert_network_packet,
    insert_network_path_node, insert_network_seen_by_node, list_message_areas, list_network_links,
    list_network_messages,
};
use oxidebbs_network::FtnAddress;
use sha2::{Digest, Sha256};

use crate::route::{FtnRouteLink, NetmailRouter};
use crate::{
    BundleExtractor, EchomailKludge, FtnError, PacketHeader, PacketMessage, PacketReader,
    RoutingDecision, duplicate_key, parse_message_body,
};

/// Filesystem paths used by the FTN tosser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TosserPaths {
    /// Manual or mailer inbound drop directory scanned for packet and bundle files.
    pub inbound_drop: PathBuf,
    /// Temporary extraction directory for bundles.
    pub temp_inbound: PathBuf,
    /// Archive directory for successfully processed inbound files.
    pub archive: PathBuf,
    /// Quarantine directory for malformed or rejected inbound files.
    pub quarantine: PathBuf,
}

impl TosserPaths {
    /// Build the default v1.2 runtime spool layout for one network profile.
    #[must_use]
    pub fn under_runtime(runtime_root: impl AsRef<Path>, network_key: &str) -> Self {
        let root = runtime_root.as_ref().join("network").join(network_key);
        Self {
            inbound_drop: root.join("inbound").join("drop"),
            temp_inbound: root.join("temp-inbound"),
            archive: root.join("inbound").join("archive"),
            quarantine: root.join("inbound").join("quarantine"),
        }
    }
}

/// Inbound FTN toss counters.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TossResult {
    pub files_processed: usize,
    pub packets_processed: usize,
    pub packets_quarantined: usize,
    pub messages_imported: usize,
    pub messages_duplicate: usize,
    pub messages_quarantined: usize,
    pub messages_skipped: usize,
    pub errors: Vec<String>,
}

impl TossResult {
    fn merge(&mut self, other: TossResult) {
        self.files_processed += other.files_processed;
        self.packets_processed += other.packets_processed;
        self.packets_quarantined += other.packets_quarantined;
        self.messages_imported += other.messages_imported;
        self.messages_duplicate += other.messages_duplicate;
        self.messages_quarantined += other.messages_quarantined;
        self.messages_skipped += other.messages_skipped;
        self.errors.extend(other.errors);
    }
}

/// DecentDB-backed FTN inbound tosser.
///
/// The tosser scans an inbound spool, validates packet origin/password against
/// configured network links, imports known echomail AREA messages into local
/// message areas, records network-message metadata, and quarantines malformed or
/// unauthorized packets for operator review.
pub struct Tosser<'db> {
    db: &'db Db,
    profile: NetworkProfileRecord,
    paths: TosserPaths,
    router: NetmailRouter,
}

impl<'db> Tosser<'db> {
    /// Create a tosser for an already-open database and network profile.
    #[must_use]
    pub fn new(db: &'db Db, profile: NetworkProfileRecord, paths: TosserPaths) -> Self {
        let router = Self::build_router(db, &profile);
        Self {
            db,
            profile,
            paths,
            router,
        }
    }

    fn build_router(db: &Db, profile: &NetworkProfileRecord) -> NetmailRouter {
        let local_address = FtnAddress {
            zone: profile.local_zone as u16,
            net: profile.local_net as u16,
            node: profile.local_node as u16,
            point: if profile.local_point == 0 {
                None
            } else {
                Some(profile.local_point as u16)
            },
        };
        let local_addresses = vec![local_address];

        let links = list_network_links(db)
            .unwrap_or_default()
            .into_iter()
            .filter(|link| link.network_id == profile.id)
            .filter_map(|link| {
                let address = link.address.parse::<FtnAddress>().ok()?;
                Some(FtnRouteLink::direct(link.key, address))
            })
            .collect();

        NetmailRouter::new(local_addresses, links)
    }

    /// Scan the inbound drop directory and process supported packet/bundle files.
    ///
    /// # Errors
    ///
    /// Returns I/O, bundle, packet, or database errors that prevent scanning the
    /// inbound directory itself. Per-file packet failures are recorded in the
    /// returned [`TossResult`] and reflected in `network_packets`.
    pub fn toss(&self) -> Result<TossResult, FtnError> {
        self.ensure_directories()?;
        let mut result = TossResult::default();
        if !self.paths.inbound_drop.exists() {
            return Ok(result);
        }

        let mut entries = fs::read_dir(&self.paths.inbound_drop)?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|entry| {
                entry
                    .file_type()
                    .map(|kind| kind.is_file())
                    .unwrap_or(false)
            })
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        entries.sort();

        for path in entries {
            match self.toss_file(&path) {
                Ok(file_result) => result.merge(file_result),
                Err(error) => {
                    result.errors.push(format!("{}: {error}", path.display()));
                    if let Ok(file_result) = self.record_rejected_file(&path, &error.to_string()) {
                        result.merge(file_result);
                    }
                }
            }
        }

        Ok(result)
    }

    /// Process one inbound packet or bundle path.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be classified, extracted, moved, or
    /// recorded. Malformed packets are converted into quarantine records and do
    /// not abort processing of other files.
    pub fn toss_file(&self, path: impl AsRef<Path>) -> Result<TossResult, FtnError> {
        self.ensure_directories()?;
        let path = path.as_ref();
        let packet_paths = BundleExtractor::extract_packets(path, &self.paths.temp_inbound)?;
        let mut result = TossResult {
            files_processed: 1,
            ..TossResult::default()
        };

        let mut file_had_quarantine = false;
        for packet_path in packet_paths {
            let packet_result = self.process_packet_path(&packet_path)?;
            file_had_quarantine |= packet_result.packets_quarantined > 0;
            result.merge(packet_result);
        }

        let destination = if file_had_quarantine {
            self.paths.quarantine.join(file_name(path)?)
        } else {
            self.paths.archive.join(file_name(path)?)
        };
        move_file(path, &destination)?;

        Ok(result)
    }

    fn process_packet_path(&self, path: &Path) -> Result<TossResult, FtnError> {
        let packet_record = self.create_packet_record(path)?;
        let packet = match PacketReader::read(File::open(path)?) {
            Ok(packet) => packet,
            Err(error) => {
                finish_network_packet(
                    self.db,
                    &packet_record.id,
                    "quarantined",
                    Some(&error.to_string()),
                )?;
                return Ok(TossResult {
                    packets_processed: 1,
                    packets_quarantined: 1,
                    errors: vec![format!("{}: {error}", path.display())],
                    ..TossResult::default()
                });
            }
        };

        let link = match self.validate_packet(&packet.header) {
            Ok(link) => link,
            Err(error) => {
                finish_network_packet(
                    self.db,
                    &packet_record.id,
                    "quarantined",
                    Some(&error.to_string()),
                )?;
                return Ok(TossResult {
                    packets_processed: 1,
                    packets_quarantined: 1,
                    errors: vec![format!("{}: {error}", path.display())],
                    ..TossResult::default()
                });
            }
        };

        let mut result = TossResult {
            packets_processed: 1,
            ..TossResult::default()
        };
        for message in &packet.messages {
            let message_result =
                self.process_message(&packet_record.id, &packet.header, &link, message)?;
            result.merge(message_result);
        }

        let packet_status = if result.messages_quarantined > 0 {
            "quarantined"
        } else {
            "processed"
        };
        let packet_error = if result.messages_quarantined > 0 {
            Some("one or more packet messages were quarantined")
        } else {
            None
        };
        finish_network_packet(self.db, &packet_record.id, packet_status, packet_error)?;
        if packet_status == "quarantined" {
            result.packets_quarantined += 1;
        }
        Ok(result)
    }

    fn process_message(
        &self,
        packet_id: &str,
        header: &PacketHeader,
        link: &NetworkLinkRecord,
        message: &PacketMessage,
    ) -> Result<TossResult, FtnError> {
        if message.area_tag.trim().is_empty() {
            return self.process_netmail(packet_id, header, message);
        }

        self.process_echomail(packet_id, header, link, message)
    }

    fn process_netmail(
        &self,
        packet_id: &str,
        header: &PacketHeader,
        message: &PacketMessage,
    ) -> Result<TossResult, FtnError> {
        let destination = destination_address(header);
        let attributes = message.attributes;
        let decision = self.router.route(&destination, attributes);

        match decision {
            RoutingDecision::LocalDelivery { .. } => {
                self.deliver_local_netmail(packet_id, header, message)
            }
            RoutingDecision::Direct { link }
            | RoutingDecision::Crash { link }
            | RoutingDecision::Hold { link } => {
                self.queue_forwarded_netmail(packet_id, header, message, &link, None)
            }
            RoutingDecision::RoutedViaHub {
                hub,
                final_destination,
            } => self.queue_forwarded_netmail(
                packet_id,
                header,
                message,
                &hub,
                Some(final_destination),
            ),
            RoutingDecision::UnknownDestination { .. } => self.record_quarantined_network_message(
                packet_id,
                header,
                message,
                "no configured route for netmail destination",
            ),
        }
    }

    fn deliver_local_netmail(
        &self,
        packet_id: &str,
        header: &PacketHeader,
        message: &PacketMessage,
    ) -> Result<TossResult, FtnError> {
        let to_user_lower = message.to_user.to_ascii_lowercase();
        if to_user_lower == "areafix" || to_user_lower == "areamgr" {
            return self.process_areafix_netmail(packet_id, header, message);
        }

        let origin = origin_address(header);
        let destination = destination_address(header);

        let Some(area) = list_message_areas(self.db)?.into_iter().find(|area| {
            area.enabled
                && area.kind == "netmail"
                && area.network_id.as_deref() == Some(self.profile.id.as_str())
        }) else {
            return self.record_quarantined_network_message(
                packet_id,
                header,
                message,
                "no enabled local netmail area is configured for this network profile",
            );
        };

        let parsed_body = parse_message_body(&String::from_utf8_lossy(&message.body));
        let msgid = parsed_body.kludges.iter().find_map(|kludge| match kludge {
            EchomailKludge::Msgid(value) => Some(value.clone()),
            _ => None,
        });
        let replyid = parsed_body.kludges.iter().find_map(|kludge| match kludge {
            EchomailKludge::Reply(value) => Some(value.clone()),
            _ => None,
        });
        let display_body = parsed_body.body_lines.join("\n");
        let created_at = current_timestamp(self.db)?;
        let duplicate_area = format!("netmail:{destination}");
        let duplicate = duplicate_key(
            self.profile.id.clone(),
            duplicate_area.clone(),
            origin.clone(),
            msgid.as_deref(),
            &message.body,
        );

        if self.is_duplicate(&duplicate)? {
            insert_network_message(
                self.db,
                &NetworkMessageRecord {
                    id: generated_uuid(self.db)?,
                    network_id: self.profile.id.clone(),
                    local_message_id: None,
                    message_type: "netmail".to_string(),
                    area_tag: None,
                    origin_address: origin.to_string(),
                    destination_address: Some(destination.to_string()),
                    from_name: nonblank(&message.from_user, "Unknown"),
                    to_name: Some(nonblank(&message.to_user, "Sysop")),
                    subject: nonblank(&message.subject, "(no subject)"),
                    raw_text: message.body.clone(),
                    display_body,
                    msgid: msgid.clone(),
                    replyid,
                    created_at: created_at.clone(),
                    imported_at: Some(created_at.clone()),
                    exported_at: None,
                    duplicate_hash: Some(duplicate.message_id.clone()),
                    packet_id: Some(packet_id.to_string()),
                    status: "duplicate".to_string(),
                },
            )?;
            insert_network_duplicate_log(
                self.db,
                &NetworkDuplicateLogRecord {
                    id: generated_uuid(self.db)?,
                    network_id: self.profile.id.clone(),
                    duplicate_hash: duplicate.message_id,
                    msgid,
                    area_tag: Some(duplicate_area),
                    origin_address: origin.to_string(),
                    detected_at: created_at,
                    action: "rejected".to_string(),
                },
            )?;
            return Ok(TossResult {
                messages_duplicate: 1,
                ..TossResult::default()
            });
        }

        let network_message_id = generated_uuid(self.db)?;
        let local_message_id = generated_uuid(self.db)?;
        insert_message(
            self.db,
            &MessageRecord {
                id: local_message_id.clone(),
                area_id: area.id,
                author_user_id: String::new(),
                author_kind: "network".to_string(),
                author_display_name: nonblank(&message.from_user, "Unknown"),
                author_network_address: Some(origin.to_string()),
                to_user_id: None,
                subject: nonblank(&message.subject, "(no subject)"),
                body: display_body.clone(),
                created_at: created_at.clone(),
                reply_to_id: None,
                network_message_id: Some(network_message_id.clone()),
                visibility: "normal".to_string(),
            },
        )?;
        insert_network_message(
            self.db,
            &NetworkMessageRecord {
                id: network_message_id,
                network_id: self.profile.id.clone(),
                local_message_id: Some(local_message_id),
                message_type: "netmail".to_string(),
                area_tag: None,
                origin_address: origin.to_string(),
                destination_address: Some(destination.to_string()),
                from_name: nonblank(&message.from_user, "Unknown"),
                to_name: Some(nonblank(&message.to_user, "Sysop")),
                subject: nonblank(&message.subject, "(no subject)"),
                raw_text: message.body.clone(),
                display_body,
                msgid,
                replyid,
                created_at: created_at.clone(),
                imported_at: Some(created_at),
                exported_at: None,
                duplicate_hash: Some(duplicate.message_id),
                packet_id: Some(packet_id.to_string()),
                status: "imported".to_string(),
            },
        )?;

        Ok(TossResult {
            messages_imported: 1,
            ..TossResult::default()
        })
    }

    fn process_areafix_netmail(
        &self,
        packet_id: &str,
        header: &PacketHeader,
        message: &PacketMessage,
    ) -> Result<TossResult, FtnError> {
        use crate::areafix::AreaFixProcessor;

        let origin = origin_address(header);
        let destination = destination_address(header);

        let Some(link) = list_network_links(self.db)?.into_iter().find(|link| {
            link.network_id == self.profile.id
                && link.address.parse::<FtnAddress>().ok() == Some(origin.clone())
        }) else {
            return self.record_quarantined_network_message(
                packet_id,
                header,
                message,
                "AreaFix request from unknown or unconfigured link",
            );
        };

        let body_text = String::from_utf8_lossy(&message.body);
        let mut lines = body_text.lines();
        let first_line = lines.next().unwrap_or("");

        let (password, command_body) = if !message.subject.is_empty() {
            (message.subject.clone(), body_text.to_string())
        } else if let Some(stripped) = first_line.strip_prefix("- ") {
            (
                stripped.trim().to_string(),
                lines.collect::<Vec<_>>().join("\n"),
            )
        } else {
            (String::new(), body_text.to_string())
        };

        let processor = AreaFixProcessor::new(self.db, self.profile.clone(), link.clone());
        let result = processor.process_request(&password, &command_body);

        let created_at = current_timestamp(self.db)?;
        let network_message_id = generated_uuid(self.db)?;

        match result {
            Ok(areafix_result) => {
                let reply_body = areafix_result.reply.into_bytes();
                let reply_packet_id = generated_uuid(self.db)?;
                let reply_filename = format!("{}.pkt", generated_uuid(self.db)?.replace('-', ""));
                let reply_sha256_bytes = Sha256::digest(&reply_body);
                let reply_sha256: String = reply_sha256_bytes
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect();

                insert_network_packet(
                    self.db,
                    &NetworkPacketRecord {
                        id: reply_packet_id.clone(),
                        network_id: self.profile.id.clone(),
                        direction: "outbound".to_string(),
                        link_id: Some(link.id.clone()),
                        filename: reply_filename,
                        sha256: reply_sha256,
                        size_bytes: reply_body.len() as i64,
                        status: "pending".to_string(),
                        error_message: None,
                        received_at: None,
                        processed_at: None,
                        created_at: created_at.clone(),
                    },
                )?;

                insert_network_message(
                    self.db,
                    &NetworkMessageRecord {
                        id: network_message_id,
                        network_id: self.profile.id.clone(),
                        local_message_id: None,
                        message_type: "netmail".to_string(),
                        area_tag: None,
                        origin_address: destination.to_string(),
                        destination_address: Some(origin.to_string()),
                        from_name: "AreaFix".to_string(),
                        to_name: Some(message.from_user.clone()),
                        subject: "AreaFix Response".to_string(),
                        raw_text: reply_body.clone(),
                        display_body: String::from_utf8_lossy(&reply_body).to_string(),
                        msgid: None,
                        replyid: None,
                        created_at: created_at.clone(),
                        imported_at: Some(created_at.clone()),
                        exported_at: None,
                        duplicate_hash: None,
                        packet_id: Some(reply_packet_id),
                        status: "pending".to_string(),
                    },
                )?;

                for rescan_area_tag in areafix_result.rescan_requests {
                    self.queue_rescan_request(&link, &rescan_area_tag, &created_at)?;
                }

                Ok(TossResult {
                    messages_imported: 1,
                    ..TossResult::default()
                })
            }
            Err(error) => {
                let error_body = format!("AreaFix error: {error}").into_bytes();
                insert_network_message(
                    self.db,
                    &NetworkMessageRecord {
                        id: network_message_id,
                        network_id: self.profile.id.clone(),
                        local_message_id: None,
                        message_type: "netmail".to_string(),
                        area_tag: None,
                        origin_address: destination.to_string(),
                        destination_address: Some(origin.to_string()),
                        from_name: "AreaFix".to_string(),
                        to_name: Some(message.from_user.clone()),
                        subject: "AreaFix Error".to_string(),
                        raw_text: error_body.clone(),
                        display_body: String::from_utf8_lossy(&error_body).to_string(),
                        msgid: None,
                        replyid: None,
                        created_at: created_at.clone(),
                        imported_at: Some(created_at),
                        exported_at: None,
                        duplicate_hash: None,
                        packet_id: Some(packet_id.to_string()),
                        status: "quarantined".to_string(),
                    },
                )?;

                Ok(TossResult {
                    messages_quarantined: 1,
                    errors: vec![error.to_string()],
                    ..TossResult::default()
                })
            }
        }
    }

    fn queue_forwarded_netmail(
        &self,
        _packet_id: &str,
        header: &PacketHeader,
        message: &PacketMessage,
        link: &FtnRouteLink,
        final_destination: Option<FtnAddress>,
    ) -> Result<TossResult, FtnError> {
        let origin = origin_address(header);
        let destination = final_destination.unwrap_or_else(|| destination_address(header));
        let created_at = current_timestamp(self.db)?;
        let link_record = list_network_links(self.db)?
            .into_iter()
            .find(|record| record.network_id == self.profile.id && record.key == link.key)
            .ok_or_else(|| {
                FtnError::Protocol(format!(
                    "routed netmail selected link {} but no matching network link row exists",
                    link.key
                ))
            })?;

        let outbound_packet_id = generated_uuid(self.db)?;
        let filename = format!("{}.pkt", generated_uuid(self.db)?.replace('-', ""));
        let sha256_bytes = Sha256::digest(&message.body);
        let sha256: String = sha256_bytes.iter().map(|b| format!("{b:02x}")).collect();

        insert_network_packet(
            self.db,
            &NetworkPacketRecord {
                id: outbound_packet_id.clone(),
                network_id: self.profile.id.clone(),
                direction: "outbound".to_string(),
                link_id: Some(link_record.id.clone()),
                filename,
                sha256,
                size_bytes: message.body.len() as i64,
                status: "pending".to_string(),
                error_message: None,
                received_at: None,
                processed_at: None,
                created_at: created_at.clone(),
            },
        )?;

        let network_message_id = generated_uuid(self.db)?;
        insert_network_message(
            self.db,
            &NetworkMessageRecord {
                id: network_message_id,
                network_id: self.profile.id.clone(),
                local_message_id: None,
                message_type: "netmail".to_string(),
                area_tag: None,
                origin_address: origin.to_string(),
                destination_address: Some(destination.to_string()),
                from_name: nonblank(&message.from_user, "Unknown"),
                to_name: Some(nonblank(&message.to_user, "Sysop")),
                subject: nonblank(&message.subject, "(no subject)"),
                raw_text: message.body.clone(),
                display_body: String::from_utf8_lossy(&message.body).to_string(),
                msgid: None,
                replyid: None,
                created_at: created_at.clone(),
                imported_at: Some(created_at),
                exported_at: None,
                duplicate_hash: None,
                packet_id: Some(outbound_packet_id),
                status: "pending".to_string(),
            },
        )?;

        Ok(TossResult {
            messages_imported: 1,
            ..TossResult::default()
        })
    }

    fn process_echomail(
        &self,
        packet_id: &str,
        header: &PacketHeader,
        _link: &NetworkLinkRecord,
        message: &PacketMessage,
    ) -> Result<TossResult, FtnError> {
        let area_tag = message.area_tag.trim();
        let Some(area) = find_network_area_by_tag_and_profile(self.db, &self.profile.id, area_tag)?
        else {
            return self.record_quarantined_network_message(
                packet_id,
                header,
                message,
                "unknown echomail AREA tag",
            );
        };

        let origin = origin_address(header);
        let parsed_body = parse_message_body(&String::from_utf8_lossy(&message.body));
        let msgid = parsed_body.kludges.iter().find_map(|kludge| match kludge {
            EchomailKludge::Msgid(value) => Some(value.clone()),
            _ => None,
        });
        let replyid = parsed_body.kludges.iter().find_map(|kludge| match kludge {
            EchomailKludge::Reply(value) => Some(value.clone()),
            _ => None,
        });
        let display_body = parsed_body.body_lines.join("\n");
        let created_at = current_timestamp(self.db)?;
        let duplicate = duplicate_key(
            self.profile.id.clone(),
            area_tag.to_string(),
            origin.clone(),
            msgid.as_deref(),
            &message.body,
        );

        if self.is_duplicate(&duplicate)? {
            let network_message_id = generated_uuid(self.db)?;
            insert_network_message(
                self.db,
                &NetworkMessageRecord {
                    id: network_message_id,
                    network_id: self.profile.id.clone(),
                    local_message_id: None,
                    message_type: "echomail".to_string(),
                    area_tag: Some(area_tag.to_string()),
                    origin_address: origin.to_string(),
                    destination_address: Some(destination_address(header).to_string()),
                    from_name: nonblank(&message.from_user, "Unknown"),
                    to_name: Some(nonblank(&message.to_user, "All")),
                    subject: nonblank(&message.subject, "(no subject)"),
                    raw_text: message.body.clone(),
                    display_body,
                    msgid: msgid.clone(),
                    replyid,
                    created_at: created_at.clone(),
                    imported_at: Some(created_at.clone()),
                    exported_at: None,
                    duplicate_hash: Some(duplicate.message_id.clone()),
                    packet_id: Some(packet_id.to_string()),
                    status: "duplicate".to_string(),
                },
            )?;
            insert_network_duplicate_log(
                self.db,
                &NetworkDuplicateLogRecord {
                    id: generated_uuid(self.db)?,
                    network_id: self.profile.id.clone(),
                    duplicate_hash: duplicate.message_id,
                    msgid,
                    area_tag: Some(area_tag.to_string()),
                    origin_address: origin.to_string(),
                    detected_at: created_at,
                    action: "rejected".to_string(),
                },
            )?;
            return Ok(TossResult {
                messages_duplicate: 1,
                ..TossResult::default()
            });
        }

        let network_message_id = generated_uuid(self.db)?;
        let local_message_id = generated_uuid(self.db)?;
        insert_message(
            self.db,
            &MessageRecord {
                id: local_message_id.clone(),
                area_id: area.local_area_id,
                author_user_id: String::new(),
                author_kind: "network".to_string(),
                author_display_name: nonblank(&message.from_user, "Unknown"),
                author_network_address: Some(origin.to_string()),
                to_user_id: None,
                subject: nonblank(&message.subject, "(no subject)"),
                body: display_body.clone(),
                created_at: created_at.clone(),
                reply_to_id: None,
                network_message_id: Some(network_message_id.clone()),
                visibility: "normal".to_string(),
            },
        )?;
        insert_network_message(
            self.db,
            &NetworkMessageRecord {
                id: network_message_id.clone(),
                network_id: self.profile.id.clone(),
                local_message_id: Some(local_message_id),
                message_type: "echomail".to_string(),
                area_tag: Some(area_tag.to_string()),
                origin_address: origin.to_string(),
                destination_address: Some(destination_address(header).to_string()),
                from_name: nonblank(&message.from_user, "Unknown"),
                to_name: Some(nonblank(&message.to_user, "All")),
                subject: nonblank(&message.subject, "(no subject)"),
                raw_text: message.body.clone(),
                display_body,
                msgid,
                replyid,
                created_at: created_at.clone(),
                imported_at: Some(created_at),
                exported_at: None,
                duplicate_hash: Some(duplicate.message_id),
                packet_id: Some(packet_id.to_string()),
                status: "imported".to_string(),
            },
        )?;
        self.store_seen_by_and_path(&network_message_id, &origin, &parsed_body.kludges)?;

        Ok(TossResult {
            messages_imported: 1,
            ..TossResult::default()
        })
    }

    fn record_quarantined_network_message(
        &self,
        packet_id: &str,
        header: &PacketHeader,
        message: &PacketMessage,
        reason: &str,
    ) -> Result<TossResult, FtnError> {
        let body = String::from_utf8_lossy(&message.body);
        let parsed_body = parse_message_body(&body);
        let created_at = current_timestamp(self.db)?;
        insert_network_message(
            self.db,
            &NetworkMessageRecord {
                id: generated_uuid(self.db)?,
                network_id: self.profile.id.clone(),
                local_message_id: None,
                message_type: if message.area_tag.trim().is_empty() {
                    "netmail".to_string()
                } else {
                    "echomail".to_string()
                },
                area_tag: (!message.area_tag.trim().is_empty()).then_some(message.area_tag.clone()),
                origin_address: origin_address(header).to_string(),
                destination_address: Some(destination_address(header).to_string()),
                from_name: nonblank(&message.from_user, "Unknown"),
                to_name: Some(nonblank(&message.to_user, "All")),
                subject: nonblank(&message.subject, "(no subject)"),
                raw_text: message.body.clone(),
                display_body: parsed_body.body_lines.join("\n"),
                msgid: parsed_body.kludges.iter().find_map(|kludge| match kludge {
                    EchomailKludge::Msgid(value) => Some(value.clone()),
                    _ => None,
                }),
                replyid: parsed_body.kludges.iter().find_map(|kludge| match kludge {
                    EchomailKludge::Reply(value) => Some(value.clone()),
                    _ => None,
                }),
                created_at: created_at.clone(),
                imported_at: Some(created_at),
                exported_at: None,
                duplicate_hash: None,
                packet_id: Some(packet_id.to_string()),
                status: "quarantined".to_string(),
            },
        )?;
        Ok(TossResult {
            messages_quarantined: 1,
            errors: vec![format!(
                "{} message from {} quarantined: {reason}",
                if message.area_tag.trim().is_empty() {
                    "netmail"
                } else {
                    "echomail"
                },
                message.from_user
            )],
            ..TossResult::default()
        })
    }

    fn store_seen_by_and_path(
        &self,
        network_message_id: &str,
        origin: &FtnAddress,
        kludges: &[EchomailKludge],
    ) -> Result<(), FtnError> {
        let mut path_sequence = 0_i64;
        for kludge in kludges {
            match kludge {
                EchomailKludge::SeenBy(raw) => {
                    for address in parse_address_list(raw, origin) {
                        insert_network_seen_by_node(
                            self.db,
                            &NetworkSeenByNode {
                                id: generated_uuid(self.db)?,
                                message_id: network_message_id.to_string(),
                                network_id: self.profile.id.clone(),
                                zone: i64::from(address.zone),
                                net: i64::from(address.net),
                                node: i64::from(address.node),
                            },
                        )?;
                    }
                }
                EchomailKludge::Path(raw) => {
                    for address in parse_address_list(raw, origin) {
                        insert_network_path_node(
                            self.db,
                            &NetworkPathNode {
                                id: generated_uuid(self.db)?,
                                message_id: network_message_id.to_string(),
                                network_id: self.profile.id.clone(),
                                sequence: path_sequence,
                                zone: i64::from(address.zone),
                                net: i64::from(address.net),
                                node: i64::from(address.node),
                            },
                        )?;
                        path_sequence += 1;
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn validate_packet(&self, header: &PacketHeader) -> Result<NetworkLinkRecord, FtnError> {
        let origin = origin_address(header);
        let destination = destination_address(header);
        let expected_destination = profile_address(&self.profile)?;
        if destination != expected_destination {
            return Err(FtnError::Protocol(format!(
                "packet destination {destination} does not match local network address {expected_destination}"
            )));
        }

        let packet_password = packet_password_text(header);
        let links = list_network_links(self.db)?;
        links
            .into_iter()
            .filter(|link| link.network_id == self.profile.id)
            .find(|link| {
                link_address_matches(link, &origin) && link_password_matches(link, &packet_password)
            })
            .ok_or_else(|| {
                FtnError::Protocol(format!(
                    "no enabled link matched packet origin {origin} and supplied password"
                ))
            })
    }

    fn create_packet_record(&self, path: &Path) -> Result<NetworkPacketRecord, FtnError> {
        let now = current_timestamp(self.db)?;
        let packet = NetworkPacketRecord {
            id: generated_uuid(self.db)?,
            network_id: self.profile.id.clone(),
            direction: "inbound".to_string(),
            link_id: None,
            filename: path.display().to_string(),
            sha256: sha256_file(path)?,
            size_bytes: fs::metadata(path)?.len().try_into().unwrap_or(i64::MAX),
            status: "pending".to_string(),
            error_message: None,
            received_at: Some(now.clone()),
            processed_at: None,
            created_at: now,
        };
        insert_network_packet(self.db, &packet)?;
        Ok(packet)
    }

    fn record_rejected_file(&self, path: &Path, reason: &str) -> Result<TossResult, FtnError> {
        let packet = self.create_packet_record(path)?;
        finish_network_packet(self.db, &packet.id, "quarantined", Some(reason))?;
        let destination = self.paths.quarantine.join(file_name(path)?);
        move_file(path, &destination)?;
        Ok(TossResult {
            files_processed: 1,
            packets_processed: 1,
            packets_quarantined: 1,
            errors: vec![format!("{}: {reason}", path.display())],
            ..TossResult::default()
        })
    }

    fn is_duplicate(
        &self,
        key: &oxidebbs_network::DuplicateDetectionKey,
    ) -> Result<bool, FtnError> {
        let logged = crate::DecentDbDuplicateDetector::new(self.db).try_is_duplicate(key)?;
        if logged {
            return Ok(true);
        }
        Ok(list_network_messages(self.db)?.into_iter().any(|message| {
            message.network_id == key.network_id
                && message.duplicate_hash.as_deref() == Some(key.message_id.as_str())
                && message.area_tag.as_deref() == Some(key.area_tag.as_str())
                && message.origin_address == key.origin.to_string()
        }))
    }

    fn ensure_directories(&self) -> Result<(), FtnError> {
        fs::create_dir_all(&self.paths.inbound_drop)?;
        fs::create_dir_all(&self.paths.temp_inbound)?;
        fs::create_dir_all(&self.paths.archive)?;
        fs::create_dir_all(&self.paths.quarantine)?;
        Ok(())
    }

    fn queue_rescan_request(
        &self,
        link: &NetworkLinkRecord,
        area_tag: &str,
        created_at: &str,
    ) -> Result<(), FtnError> {
        let rescan_id = generated_uuid(self.db)?;
        let rescan_record = oxidebbs_db::NetworkRescanQueueRecord {
            id: rescan_id,
            network_id: self.profile.id.clone(),
            link_id: link.id.clone(),
            area_tag: area_tag.to_string(),
            status: "pending".to_string(),
            requested_at: created_at.to_string(),
            processed_at: None,
        };
        oxidebbs_db::insert_network_rescan_queue(self.db, &rescan_record)?;
        Ok(())
    }
}

fn origin_address(header: &PacketHeader) -> FtnAddress {
    FtnAddress {
        zone: nonzero_or(header.orig_zone2, header.orig_zone),
        net: nonzero_or(header.orig_net2, header.orig_net),
        node: header.orig_node,
        point: None,
    }
}

fn destination_address(header: &PacketHeader) -> FtnAddress {
    FtnAddress {
        zone: nonzero_or(header.dest_zone2, header.dest_zone),
        net: nonzero_or(header.dest_net2, header.dest_net),
        node: header.dest_node,
        point: None,
    }
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

fn nonzero_or(preferred: u16, fallback: u16) -> u16 {
    if preferred == 0 { fallback } else { preferred }
}

fn packet_password_text(header: &PacketHeader) -> String {
    String::from_utf8_lossy(&header.password)
        .trim_matches(char::from(0))
        .trim()
        .to_string()
}

fn link_address_matches(link: &NetworkLinkRecord, origin: &FtnAddress) -> bool {
    link.enabled
        && link
            .address
            .parse::<FtnAddress>()
            .is_ok_and(|address| address == *origin)
}

fn link_password_matches(link: &NetworkLinkRecord, packet_password: &str) -> bool {
    link.password
        .trim()
        .eq_ignore_ascii_case(packet_password.trim())
}

fn parse_address_list(raw: &str, default: &FtnAddress) -> Vec<FtnAddress> {
    raw.split_whitespace()
        .filter_map(|token| parse_seen_by_or_path_token(token, default))
        .collect()
}

fn parse_seen_by_or_path_token(token: &str, default: &FtnAddress) -> Option<FtnAddress> {
    if let Ok(address) = token.parse::<FtnAddress>() {
        return Some(address);
    }
    let (net, node) = token.split_once('/')?;
    Some(FtnAddress {
        zone: default.zone,
        net: net.parse().ok()?,
        node: node.parse().ok()?,
        point: None,
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

fn move_file(source: &Path, destination: &Path) -> Result<(), FtnError> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    let destination = available_destination(destination);
    fs::rename(source, destination)?;
    Ok(())
}

fn available_destination(destination: &Path) -> PathBuf {
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

fn file_name(path: &Path) -> Result<&std::ffi::OsStr, FtnError> {
    path.file_name().ok_or_else(|| {
        FtnError::Protocol(format!("inbound file has no filename: {}", path.display()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FtnPacket, MessageAttribute, PacketWriter};
    use oxidebbs_db::{
        MessageAreaRecord, NetworkAreaRecord, OxideDb, insert_message_area, insert_network_area,
        insert_network_link, insert_network_profile, list_messages, list_network_packets,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    const PROFILE_ID: &str = "00000000-0000-4000-8000-000000001001";
    const LINK_ID: &str = "00000000-0000-4000-8000-000000001002";
    const AREA_ID: &str = "00000000-0000-4000-8000-000000001003";
    const NETWORK_AREA_ID: &str = "00000000-0000-4000-8000-000000001004";

    fn test_db() -> OxideDb {
        let db = OxideDb::open_memory().expect("open db");
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
        insert_network_link(db.db(), &link("SECRET")).expect("insert link");
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
        db
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

    fn link(password: &str) -> NetworkLinkRecord {
        NetworkLinkRecord {
            id: LINK_ID.to_string(),
            key: "hub".to_string(),
            network_id: PROFILE_ID.to_string(),
            address: "1:105/1".to_string(),
            host: "hub.example".to_string(),
            binkp_port: 24554,
            password: password.to_string(),
            poll_schedule_minutes: 60,
            compression: "none".to_string(),
            transport_security: "plaintext_legacy".to_string(),
            enabled: true,
            created_at: "2026-06-04T00:00:00Z".to_string(),
            updated_at: "2026-06-04T00:00:00Z".to_string(),
        }
    }

    fn temp_root(test_name: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        std::env::temp_dir().join(format!("oxidebbs-tosser-{test_name}-{suffix}"))
    }

    fn packet_path(root: &Path, name: &str, password: &str, body: &[u8]) -> PathBuf {
        packet_path_with_messages(root, name, password, vec![body.to_vec()])
    }

    fn packet_path_with_messages(
        root: &Path,
        name: &str,
        password: &str,
        bodies: Vec<Vec<u8>>,
    ) -> PathBuf {
        let path = root
            .join("network")
            .join("fidonet")
            .join("inbound")
            .join("drop")
            .join(name);
        fs::create_dir_all(path.parent().expect("packet parent")).expect("create inbound");
        let messages: Vec<PacketMessage> = bodies
            .into_iter()
            .enumerate()
            .map(|(i, body)| PacketMessage {
                to_user: "All".to_string(),
                from_user: format!("User{}", i),
                subject: format!("Subject {}", i),
                body,
                area_tag: "OXIDE.GENERAL".to_string(),
                attributes: MessageAttribute::NONE,
            })
            .collect();
        let packet = FtnPacket {
            header: header(password),
            messages,
        };
        let mut bytes = Vec::new();
        PacketWriter::write(&mut bytes, &packet).expect("write packet");
        fs::write(&path, bytes).expect("write packet file");
        path
    }

    fn header(password: &str) -> PacketHeader {
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
    fn tosses_known_echomail_packet_into_local_area() {
        let db = test_db();
        let root = temp_root("good");
        let _packet = packet_path(
            &root,
            "00000001.pkt",
            "SECRET",
            b"AREA:OXIDE.GENERAL\r\x01MSGID: 1:105/1 abc\rHello from FTN\rSEEN-BY: 105/1\rPATH: 105/1\r",
        );
        let tosser = Tosser::new(
            db.db(),
            profile(),
            TosserPaths::under_runtime(&root, "fidonet"),
        );

        let result = tosser.toss().expect("toss");

        assert_eq!(result.messages_imported, 1);
        assert_eq!(result.packets_quarantined, 0);
        let messages = list_messages(db.db()).expect("list messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].author_kind, "network");
        assert_eq!(
            messages[0].author_network_address.as_deref(),
            Some("1:105/1")
        );
        assert_eq!(messages[0].body, "Hello from FTN");
        assert!(
            root.join("network/fidonet/inbound/archive/00000001.pkt")
                .exists()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn wrong_password_quarantines_packet_without_importing_messages() {
        let db = test_db();
        let root = temp_root("bad-password");
        let _packet = packet_path(
            &root,
            "00000002.pkt",
            "WRONG",
            b"AREA:OXIDE.GENERAL\r\x01MSGID: 1:105/1 abc\rHello\r",
        );
        let tosser = Tosser::new(
            db.db(),
            profile(),
            TosserPaths::under_runtime(&root, "fidonet"),
        );

        let result = tosser.toss().expect("toss");

        assert_eq!(result.messages_imported, 0);
        assert_eq!(result.packets_quarantined, 1);
        assert!(list_messages(db.db()).expect("list messages").is_empty());
        let packets = list_network_packets(db.db()).expect("list packets");
        assert_eq!(packets[0].status, "quarantined");
        assert!(
            root.join("network/fidonet/inbound/quarantine/00000002.pkt")
                .exists()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn duplicate_packet_message_is_logged_and_skipped() {
        let db = test_db();
        let root = temp_root("duplicate");
        let body = b"AREA:OXIDE.GENERAL\r\x01MSGID: 1:105/1 duplicate\rHello\r";
        let _first = packet_path(&root, "00000003.pkt", "SECRET", body);
        let tosser = Tosser::new(
            db.db(),
            profile(),
            TosserPaths::under_runtime(&root, "fidonet"),
        );
        let first = tosser.toss().expect("first toss");
        assert_eq!(first.messages_imported, 1);

        let _second = packet_path(&root, "00000004.pkt", "SECRET", body);
        let second = tosser.toss().expect("second toss");

        assert_eq!(second.messages_imported, 0);
        assert_eq!(second.messages_duplicate, 1);
        assert_eq!(list_messages(db.db()).expect("list messages").len(), 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stress_test_1000_message_packet() {
        let db = test_db();
        let root = temp_root("stress-1000-msg");

        // Create packet with 1000 messages
        let bodies: Vec<Vec<u8>> = (0..1000)
            .map(|i| {
                format!(
                    "AREA:OXIDE.GENERAL\r\x01MSGID: 1:105/1 msg{}\rBody {}\rSEEN-BY: 105/1\rPATH: 105/1\r",
                    i, i
                )
                .into_bytes()
            })
            .collect();

        let _packet = packet_path_with_messages(&root, "large.pkt", "SECRET", bodies);
        let tosser = Tosser::new(
            db.db(),
            profile(),
            TosserPaths::under_runtime(&root, "fidonet"),
        );

        let start = std::time::Instant::now();
        let result = tosser.toss().expect("toss");
        let elapsed = start.elapsed();

        assert_eq!(result.messages_imported, 1000);
        assert_eq!(result.packets_processed, 1);
        assert_eq!(result.packets_quarantined, 0);

        let messages = list_messages(db.db()).expect("list messages");
        assert_eq!(messages.len(), 1000);

        println!("1000-message packet toss: {:?}", elapsed);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stress_test_100_packets_one_toss() {
        let db = test_db();
        let root = temp_root("stress-100-pkt");

        // Create 100 packet files
        for i in 0..100 {
            let body = format!(
                "AREA:OXIDE.GENERAL\r\x01MSGID: 1:105/1 unique{}\rBody {}\rSEEN-BY: 105/1\rPATH: 105/1\r",
                i, i
            );
            let _packet = packet_path(&root, &format!("{:08}.pkt", i), "SECRET", body.as_bytes());
        }

        let tosser = Tosser::new(
            db.db(),
            profile(),
            TosserPaths::under_runtime(&root, "fidonet"),
        );

        let start = std::time::Instant::now();
        let result = tosser.toss().expect("toss");
        let elapsed = start.elapsed();

        assert_eq!(result.packets_processed, 100);
        assert_eq!(result.messages_imported, 100);
        assert_eq!(result.packets_quarantined, 0);

        let messages = list_messages(db.db()).expect("list messages");
        assert_eq!(messages.len(), 100);

        println!("100 packets toss: {:?}", elapsed);
        let _ = fs::remove_dir_all(root);
    }
}
