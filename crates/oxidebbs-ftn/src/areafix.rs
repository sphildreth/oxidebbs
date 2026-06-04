use thiserror::Error;

use oxidebbs_db::{
    AuditEventRecord, Db, NetworkAreaRecord, NetworkLinkRecord, NetworkProfileRecord,
    NetworkSubscriptionRecord, Value, find_network_area_by_tag_and_profile, insert_audit_event,
    insert_network_subscription, list_network_areas, list_network_subscriptions,
    set_network_area_subscribed, set_network_subscription_status,
};

use crate::FtnError;

/// Parsed AreaFix command from an inbound netmail body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AreaFixCommand {
    /// List all areas available to the requesting link.
    List,
    /// List areas currently subscribed by the requesting link.
    Query,
    /// Return AreaFix help text.
    Help,
    /// Subscribe to an area, optionally requesting a rescan.
    Subscribe { area_tag: String, rescan: bool },
    /// Unsubscribe from an area.
    Unsubscribe { area_tag: String },
}

/// AreaFix command parsing error.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AreaFixParseError {
    /// The command body contained no actionable commands.
    #[error("AreaFix request contains no commands")]
    NoCommands,

    /// A management command such as `%LIST` had unsupported trailing text.
    #[error("AreaFix command {command:?} has unexpected trailing text")]
    UnexpectedTrailingText { command: String },

    /// An area subscription command did not include an area tag.
    #[error("AreaFix command {command:?} is missing an area tag")]
    MissingAreaTag { command: String },

    /// An area tag contained unsupported characters.
    #[error("AreaFix area tag {area_tag:?} is invalid")]
    InvalidAreaTag { area_tag: String },

    /// An AreaFix command was not recognized.
    #[error("unknown AreaFix command {command:?}")]
    UnknownCommand { command: String },
}

/// Result of processing one authenticated AreaFix request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AreaFixProcessResult {
    pub reply: String,
    pub commands_processed: usize,
    pub rescan_requests: Vec<String>,
}

/// DecentDB-backed AreaFix command executor.
///
/// The processor is shared by the local `net areafix send` command and inbound
/// netmail handling so password checks, subscription mutations, reply text, and
/// audit events stay consistent.
pub struct AreaFixProcessor<'db> {
    db: &'db Db,
    profile: NetworkProfileRecord,
    link: NetworkLinkRecord,
}

impl<'db> AreaFixProcessor<'db> {
    /// Create an AreaFix processor for one local profile and linked remote node.
    #[must_use]
    pub fn new(
        db: &'db Db,
        profile: NetworkProfileRecord,
        link: NetworkLinkRecord,
    ) -> Self {
        Self { db, profile, link }
    }

    /// Authenticate, parse, execute, audit, and render one AreaFix request.
    ///
    /// # Errors
    ///
    /// Returns a protocol error for wrong passwords, malformed commands,
    /// profile/link mismatches, and unknown area tags. Database failures are
    /// returned as [`FtnError::Database`].
    pub fn process_request(
        &self,
        supplied_password: &str,
        command_body: &str,
    ) -> Result<AreaFixProcessResult, FtnError> {
        if self.link.network_id != self.profile.id {
            return Err(FtnError::Protocol(format!(
                "link {} belongs to a different network profile",
                self.link.key
            )));
        }
        if self.link.password != supplied_password {
            self.audit(
                "network:areafix:auth-failed",
                &format!("AreaFix authentication failed for link {}", self.link.key),
            )?;
            return Err(FtnError::Protocol(
                "AreaFix password did not match the configured link password".to_string(),
            ));
        }

        let commands = parse_areafix_commands(command_body)
            .map_err(|error| FtnError::Protocol(error.to_string()))?;
        self.execute_commands(&commands)
    }

    /// Execute already-authenticated AreaFix commands.
    ///
    /// # Errors
    ///
    /// Returns a protocol error for profile/link mismatches and unknown area
    /// tags. Database failures are returned as [`FtnError::Database`].
    pub fn execute_commands(
        &self,
        commands: &[AreaFixCommand],
    ) -> Result<AreaFixProcessResult, FtnError> {
        if self.link.network_id != self.profile.id {
            return Err(FtnError::Protocol(format!(
                "link {} belongs to a different network profile",
                self.link.key
            )));
        }

        let mut lines = vec![
            format!("AreaFix response for {}", self.link.address),
            format!("Network: {}", self.profile.key),
            String::new(),
        ];
        let mut rescan_requests = Vec::new();

        for command in commands {
            match command {
                AreaFixCommand::List => {
                    lines.push("Available areas:".to_string());
                    for area in self.matching_areas()? {
                        lines.push(format!(
                            "{}\t{}\t{}",
                            area.area_tag,
                            if area.subscribed {
                                "active"
                            } else {
                                "available"
                            },
                            area.description
                        ));
                    }
                }
                AreaFixCommand::Query => {
                    lines.push("Subscribed areas:".to_string());
                    let subscriptions = self.link_subscribed_areas()?;
                    if subscriptions.is_empty() {
                        lines.push("(none)".to_string());
                    } else {
                        lines.extend(subscriptions.into_iter().map(|area| area.area_tag));
                    }
                }
                AreaFixCommand::Help => {
                    lines.push(
                        "Commands: %LIST, %QUERY, %HELP, +AREA.TAG, -AREA.TAG, +AREA.TAG !"
                            .to_string(),
                    );
                }
                AreaFixCommand::Subscribe { area_tag, rescan } => {
                    let area = self.require_area(area_tag)?;
                    self.set_link_subscription(&area, true)?;
                    self.audit(
                        "network:areafix:subscribe",
                        &format!(
                            "AreaFix subscribed link {} to area {} on network {}",
                            self.link.key, area.area_tag, self.profile.key
                        ),
                    )?;
                    lines.push(format!("Subscribed {}", area.area_tag));
                    if *rescan {
                        rescan_requests.push(area.area_tag.clone());
                        lines.push(format!(
                            "Rescan requested for {}; rescan queueing is not implemented yet",
                            area.area_tag
                        ));
                    }
                }
                AreaFixCommand::Unsubscribe { area_tag } => {
                    let area = self.require_area(area_tag)?;
                    self.set_link_subscription(&area, false)?;
                    self.audit(
                        "network:areafix:unsubscribe",
                        &format!(
                            "AreaFix unsubscribed link {} from area {} on network {}",
                            self.link.key, area.area_tag, self.profile.key
                        ),
                    )?;
                    lines.push(format!("Unsubscribed {}", area.area_tag));
                }
            }
        }

        self.audit(
            "network:areafix:processed",
            &format!(
                "processed {} AreaFix command(s) for link {} on network {}",
                commands.len(),
                self.link.key,
                self.profile.key
            ),
        )?;

        Ok(AreaFixProcessResult {
            reply: lines.join("\n"),
            commands_processed: commands.len(),
            rescan_requests,
        })
    }

    fn require_area(&self, area_tag: &str) -> Result<NetworkAreaRecord, FtnError> {
        find_network_area_by_tag_and_profile(self.db, &self.profile.id, area_tag)?.ok_or_else(
            || {
                FtnError::Protocol(format!(
                    "network area {area_tag:?} was not found for network {}",
                    self.profile.key
                ))
            },
        )
    }

    fn set_link_subscription(
        &self,
        area: &NetworkAreaRecord,
        subscribed: bool,
    ) -> Result<(), FtnError> {
        let timestamp = current_timestamp(self.db)?;
        if !set_network_subscription_status(
            self.db,
            &area.id,
            &self.link.id,
            subscribed,
            &timestamp,
            "areafix",
        )? {
            insert_network_subscription(
                self.db,
                &NetworkSubscriptionRecord {
                    id: generated_uuid(self.db)?,
                    area_id: area.id.clone(),
                    link_id: self.link.id.clone(),
                    subscribed,
                    subscribed_at: timestamp.clone(),
                    unsubscribed_at: (!subscribed).then_some(timestamp),
                    source: "areafix".to_string(),
                },
            )?;
        }

        let area_subscribed = subscribed
            || list_network_subscriptions(self.db)?
                .into_iter()
                .any(|subscription| subscription.area_id == area.id && subscription.subscribed);
        set_network_area_subscribed(self.db, &area.id, area_subscribed)?;
        Ok(())
    }

    fn link_subscribed_areas(&self) -> Result<Vec<NetworkAreaRecord>, FtnError> {
        let subscriptions = list_network_subscriptions(self.db)?;
        Ok(self
            .matching_areas()?
            .into_iter()
            .filter(|area| {
                subscriptions.iter().any(|subscription| {
                    subscription.link_id == self.link.id
                        && subscription.area_id == area.id
                        && subscription.subscribed
                })
            })
            .collect())
    }

    fn matching_areas(&self) -> Result<Vec<NetworkAreaRecord>, FtnError> {
        Ok(list_network_areas(self.db)?
            .into_iter()
            .filter(|area| area.network_id == self.profile.id)
            .collect())
    }

    fn audit(&self, event_type: &str, details: &str) -> Result<(), FtnError> {
        insert_audit_event(
            self.db,
            &AuditEventRecord {
                id: String::new(),
                created_at: String::new(),
                event_type: event_type.to_string(),
                user_id: None,
                node_number: None,
                details: details.to_string(),
            },
        )?;
        Ok(())
    }
}

/// Parse all AreaFix commands from a netmail body.
///
/// Commands are case-insensitive and area tags are normalized to uppercase
/// ASCII. Blank lines are ignored. Runtime processors are responsible for
/// password authentication, DecentDB subscription changes, reply netmail, and
/// activity logging.
///
/// # Errors
///
/// Returns a typed parse error when any nonblank line is malformed or the body
/// has no actionable commands.
pub fn parse_areafix_commands(body: &str) -> Result<Vec<AreaFixCommand>, AreaFixParseError> {
    let mut commands = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        commands.push(parse_areafix_command(trimmed)?);
    }

    if commands.is_empty() {
        return Err(AreaFixParseError::NoCommands);
    }

    Ok(commands)
}

/// Parse one AreaFix command line.
///
/// # Errors
///
/// Returns a typed parse error when the command is unknown or malformed.
pub fn parse_areafix_command(command: &str) -> Result<AreaFixCommand, AreaFixParseError> {
    let command = command.trim();
    if let Some(management) = command.strip_prefix('%') {
        return parse_management_command(command, management);
    }
    if let Some(area_tag) = command.strip_prefix('+') {
        return parse_area_command(command, area_tag, true);
    }
    if let Some(area_tag) = command.strip_prefix('-') {
        return parse_area_command(command, area_tag, false);
    }

    Err(AreaFixParseError::UnknownCommand {
        command: command.to_string(),
    })
}

fn parse_management_command(
    original: &str,
    management: &str,
) -> Result<AreaFixCommand, AreaFixParseError> {
    let mut parts = management.split_whitespace();
    let keyword = parts
        .next()
        .ok_or_else(|| AreaFixParseError::UnknownCommand {
            command: original.to_string(),
        })?;
    if parts.next().is_some() {
        return Err(AreaFixParseError::UnexpectedTrailingText {
            command: original.to_string(),
        });
    }

    match keyword.to_ascii_uppercase().as_str() {
        "LIST" => Ok(AreaFixCommand::List),
        "QUERY" => Ok(AreaFixCommand::Query),
        "HELP" => Ok(AreaFixCommand::Help),
        _ => Err(AreaFixParseError::UnknownCommand {
            command: original.to_string(),
        }),
    }
}

fn parse_area_command(
    original: &str,
    rest: &str,
    subscribe: bool,
) -> Result<AreaFixCommand, AreaFixParseError> {
    let mut parts = rest.split_whitespace();
    let raw_area_tag = parts
        .next()
        .ok_or_else(|| AreaFixParseError::MissingAreaTag {
            command: original.to_string(),
        })?;
    let area_tag = normalize_area_tag(raw_area_tag)?;
    let next_part = parts.next();
    let rescan = if subscribe {
        match next_part {
            Some("!") => true,
            Some(_) => {
                return Err(AreaFixParseError::UnexpectedTrailingText {
                    command: original.to_string(),
                });
            }
            None => false,
        }
    } else {
        if next_part.is_some() {
            return Err(AreaFixParseError::UnexpectedTrailingText {
                command: original.to_string(),
            });
        }
        false
    };

    if parts.next().is_some() {
        return Err(AreaFixParseError::UnexpectedTrailingText {
            command: original.to_string(),
        });
    }

    if subscribe {
        Ok(AreaFixCommand::Subscribe { area_tag, rescan })
    } else {
        Ok(AreaFixCommand::Unsubscribe { area_tag })
    }
}

fn normalize_area_tag(raw: &str) -> Result<String, AreaFixParseError> {
    if raw.is_empty()
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(AreaFixParseError::InvalidAreaTag {
            area_tag: raw.to_string(),
        });
    }

    Ok(raw.to_ascii_uppercase())
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

#[cfg(test)]
mod tests {
    use super::*;
    use oxidebbs_db::{
        MessageAreaRecord, OxideDb, insert_message_area, insert_network_area,
        insert_network_link, insert_network_profile, list_audit_events,
        list_network_subscriptions,
    };

    const PROFILE_ID: &str = "00000000-0000-4000-8000-000000003001";
    const LINK_ID: &str = "00000000-0000-4000-8000-000000003002";
    const AREA_ID: &str = "00000000-0000-4000-8000-000000003003";
    const NETWORK_AREA_ID: &str = "00000000-0000-4000-8000-000000003004";
    const CREATED_AT: &str = "2026-06-04T00:00:00Z";

    #[test]
    fn parses_management_commands_case_insensitively() {
        assert_eq!(parse_areafix_command("%list"), Ok(AreaFixCommand::List));
        assert_eq!(parse_areafix_command("%Query"), Ok(AreaFixCommand::Query));
        assert_eq!(parse_areafix_command("%HELP"), Ok(AreaFixCommand::Help));
    }

    #[test]
    fn parses_subscribe_unsubscribe_and_rescan_commands() {
        assert_eq!(
            parse_areafix_command("+fsx_gen"),
            Ok(AreaFixCommand::Subscribe {
                area_tag: "FSX_GEN".to_string(),
                rescan: false,
            })
        );
        assert_eq!(
            parse_areafix_command("+fsx_gen !"),
            Ok(AreaFixCommand::Subscribe {
                area_tag: "FSX_GEN".to_string(),
                rescan: true,
            })
        );
        assert_eq!(
            parse_areafix_command("-fsx_gen"),
            Ok(AreaFixCommand::Unsubscribe {
                area_tag: "FSX_GEN".to_string(),
            })
        );
    }

    #[test]
    fn parses_multiline_body_and_skips_blank_lines() {
        let commands =
            parse_areafix_commands("\n%list\n\n+retro.bbs !\n-query\n").expect("parse commands");

        assert_eq!(
            commands,
            vec![
                AreaFixCommand::List,
                AreaFixCommand::Subscribe {
                    area_tag: "RETRO.BBS".to_string(),
                    rescan: true,
                },
                AreaFixCommand::Unsubscribe {
                    area_tag: "QUERY".to_string(),
                },
            ]
        );
    }

    #[test]
    fn rejects_empty_request() {
        assert_eq!(
            parse_areafix_commands(" \n\t\n"),
            Err(AreaFixParseError::NoCommands)
        );
    }

    #[test]
    fn rejects_unknown_management_command() {
        assert_eq!(
            parse_areafix_command("%BOGUS"),
            Err(AreaFixParseError::UnknownCommand {
                command: "%BOGUS".to_string(),
            })
        );
    }

    #[test]
    fn rejects_management_trailing_text() {
        assert_eq!(
            parse_areafix_command("%LIST now"),
            Err(AreaFixParseError::UnexpectedTrailingText {
                command: "%LIST now".to_string(),
            })
        );
    }

    #[test]
    fn rejects_missing_area_tag() {
        assert_eq!(
            parse_areafix_command("+"),
            Err(AreaFixParseError::MissingAreaTag {
                command: "+".to_string(),
            })
        );
    }

    #[test]
    fn rejects_invalid_area_tag() {
        assert_eq!(
            parse_areafix_command("+BAD/TAG"),
            Err(AreaFixParseError::InvalidAreaTag {
                area_tag: "BAD/TAG".to_string(),
            })
        );
    }

    #[test]
    fn rejects_unsubscribe_rescan_marker() {
        assert_eq!(
            parse_areafix_command("-FSX_GEN !"),
            Err(AreaFixParseError::UnexpectedTrailingText {
                command: "-FSX_GEN !".to_string(),
            })
        );
    }

    #[test]
    fn rejects_extra_subscribe_arguments() {
        assert_eq!(
            parse_areafix_command("+FSX_GEN ! extra"),
            Err(AreaFixParseError::UnexpectedTrailingText {
                command: "+FSX_GEN ! extra".to_string(),
            })
        );
    }

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
        .expect("insert local area");
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
                subscribed: false,
                created_at: CREATED_AT.to_string(),
                updated_at: CREATED_AT.to_string(),
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
            created_at: CREATED_AT.to_string(),
            updated_at: CREATED_AT.to_string(),
        }
    }

    fn link() -> NetworkLinkRecord {
        NetworkLinkRecord {
            id: LINK_ID.to_string(),
            key: "hub".to_string(),
            network_id: PROFILE_ID.to_string(),
            address: "1:105/1".to_string(),
            host: "hub.example".to_string(),
            binkp_port: 24554,
            password: "SECRET".to_string(),
            poll_schedule_minutes: 60,
            compression: "none".to_string(),
            transport_security: "plaintext_legacy".to_string(),
            enabled: true,
            created_at: CREATED_AT.to_string(),
            updated_at: CREATED_AT.to_string(),
        }
    }

    #[test]
    fn processor_subscribes_and_renders_query_reply() {
        let db = test_db();
        let processor = AreaFixProcessor::new(db.db(), profile(), link());

        let result = processor
            .process_request("SECRET", "+oxide.general\n%query")
            .expect("process request");

        assert_eq!(result.commands_processed, 2);
        assert!(result.reply.contains("Subscribed OXIDE.GENERAL"));
        assert!(result.reply.contains("Subscribed areas:"));
        assert!(result.reply.contains("OXIDE.GENERAL"));
        let subscriptions = list_network_subscriptions(db.db()).expect("subscriptions");
        assert_eq!(subscriptions.len(), 1);
        assert!(subscriptions[0].subscribed);
        assert_eq!(subscriptions[0].source, "areafix");
    }

    #[test]
    fn processor_records_rescan_request_but_does_not_queue_it_yet() {
        let db = test_db();
        let processor = AreaFixProcessor::new(db.db(), profile(), link());

        let result = processor
            .process_request("SECRET", "+oxide.general !")
            .expect("process request");

        assert_eq!(result.rescan_requests, vec!["OXIDE.GENERAL".to_string()]);
        assert!(
            result.reply.contains(
                "Rescan requested for OXIDE.GENERAL; rescan queueing is not implemented yet"
            )
        );
    }

    #[test]
    fn processor_rejects_wrong_password_and_audits_failure() {
        let db = test_db();
        let processor = AreaFixProcessor::new(db.db(), profile(), link());

        let error = processor
            .process_request("WRONG", "+oxide.general")
            .expect_err("wrong password rejected");

        assert!(error.to_string().contains("AreaFix password did not match"));
        let events = list_audit_events(db.db(), 10).expect("audit events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "network:areafix:auth-failed");
    }
}
