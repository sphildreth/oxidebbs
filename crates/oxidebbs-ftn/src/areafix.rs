use thiserror::Error;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
