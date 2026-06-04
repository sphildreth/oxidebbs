use oxidebbs_network::FtnAddress;
use thiserror::Error;

use crate::error::FtnError;

/// Parsed FTN nodelist entry that can be indexed by OxideBBS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtnNodelistEntry {
    pub address: FtnAddress,
    pub name: Option<String>,
    pub location: Option<String>,
    pub sysop_name: Option<String>,
    pub phone: Option<String>,
    pub speed: Option<String>,
    pub flags: Vec<String>,
    pub raw_entry: String,
}

/// Parses common FTN nodelist rows.
///
/// Administrative `Zone`, `Region`, and `Host` rows update parser context but
/// are not emitted because OxideBBS stores only concrete nonzero node entries.
///
/// # Errors
///
/// Returns a parse error when a concrete node or point row cannot be mapped to
/// an address with the surrounding nodelist context.
pub fn parse_nodelist(input: &str) -> Result<Vec<FtnNodelistEntry>, FtnError> {
    let mut parser = NodelistParser::default();
    let mut entries = Vec::new();

    for (line_index, line) in input.lines().enumerate() {
        if let Some(entry) = parser.parse_line(line, line_index + 1)? {
            entries.push(entry);
        }
    }

    Ok(entries)
}

/// Errors raised while applying an FTS-style `NODEDIFF.xxx` file.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NodelistDiffError {
    #[error("base nodelist is empty")]
    EmptyBase,

    #[error("nodediff is empty")]
    EmptyDiff,

    #[error(
        "nodediff header does not match base nodelist header: expected {expected:?}, got {actual:?}"
    )]
    HeaderMismatch { expected: String, actual: String },

    #[error("nodediff header CRC {expected} does not match base nodelist CRC {actual}")]
    CrcMismatch { expected: u16, actual: u16 },

    #[error("nodediff header is missing CRC value")]
    MissingCrc,

    #[error("nodediff header has invalid CRC format: {value:?}")]
    InvalidCrcFormat { value: String },

    #[error("nodediff command on line {line_number} is empty")]
    EmptyCommand { line_number: usize },

    #[error("unsupported nodediff command {command:?} on line {line_number}")]
    UnsupportedCommand { line_number: usize, command: String },

    #[error("nodediff command {command:?} on line {line_number} has invalid count {count:?}")]
    InvalidCommandCount {
        line_number: usize,
        command: char,
        count: String,
    },

    #[error(
        "nodediff command {command:?} on line {line_number} needs {requested} input lines but only {remaining} remain"
    )]
    InputExhausted {
        line_number: usize,
        command: char,
        requested: usize,
        remaining: usize,
    },

    #[error(
        "nodediff add command on line {line_number} needs {requested} data lines but only {remaining} remain"
    )]
    AddDataExhausted {
        line_number: usize,
        requested: usize,
        remaining: usize,
    },

    #[error("nodediff ended with {remaining} base nodelist lines unapplied")]
    UnappliedBaseLines { remaining: usize },
}

/// Apply a conservative FTS-style `NODEDIFF.xxx` to a full nodelist text.
///
/// The supported format is the historical count-command form described in
/// FTS-5000:
///
/// - line 1 of the diff must exactly match line 1 of the base nodelist
/// - `A<count>` adds the following `<count>` diff lines to the output
/// - `C<count>` copies `<count>` unchanged base lines to the output
/// - `D<count>` deletes `<count>` base lines
///
/// The returned nodelist uses `\n` line separators.
///
/// # Errors
///
/// Returns typed errors for header mismatches, unsupported commands, invalid
/// counts, add-data underflow, copy/delete underflow, or a diff that leaves base
/// lines unapplied.
pub fn apply_nodelist_diff(base: &str, diff: &str) -> Result<String, NodelistDiffError> {
    apply_nodelist_diff_with_options(base, diff, false)
}

/// Apply a conservative FTS-style `NODEDIFF.xxx` to a full nodelist text with options.
///
/// When `validate_crc` is true, the CRC value in the diff header (if present) will
/// be validated against the calculated CRC of the base content.
///
/// See [`apply_nodelist_diff`] for full documentation.
pub fn apply_nodelist_diff_with_options(
    base: &str,
    diff: &str,
    validate_crc: bool,
) -> Result<String, NodelistDiffError> {
    let base_lines = normalized_lines(base);
    if base_lines.is_empty() {
        return Err(NodelistDiffError::EmptyBase);
    }

    let diff_lines = normalized_lines(diff);
    if diff_lines.is_empty() {
        return Err(NodelistDiffError::EmptyDiff);
    }

    if diff_lines[0] != base_lines[0] {
        return Err(NodelistDiffError::HeaderMismatch {
            expected: base_lines[0].clone(),
            actual: diff_lines[0].clone(),
        });
    }

    // Validate CRC if requested and present in header
    if validate_crc && let Some(expected_crc) = parse_header_crc(&diff_lines[0])? {
        let actual_crc = calculate_nodelist_crc(&base_lines);
        if expected_crc != actual_crc {
            return Err(NodelistDiffError::CrcMismatch {
                expected: expected_crc,
                actual: actual_crc,
            });
        }
    }

    let mut output = Vec::new();
    let mut base_index = 0;
    let mut diff_index = 1;
    while diff_index < diff_lines.len() {
        let line_number = diff_index + 1;
        let command = parse_diff_command(&diff_lines[diff_index], line_number)?;
        diff_index += 1;

        match command.kind {
            'A' => {
                let available = diff_lines.len().saturating_sub(diff_index);
                if available < command.count {
                    return Err(NodelistDiffError::AddDataExhausted {
                        line_number,
                        requested: command.count,
                        remaining: available,
                    });
                }
                output.extend_from_slice(&diff_lines[diff_index..diff_index + command.count]);
                diff_index += command.count;
            }
            'C' => {
                ensure_base_lines(
                    &base_lines,
                    base_index,
                    command.count,
                    line_number,
                    command.kind,
                )?;
                output.extend_from_slice(&base_lines[base_index..base_index + command.count]);
                base_index += command.count;
            }
            'D' => {
                ensure_base_lines(
                    &base_lines,
                    base_index,
                    command.count,
                    line_number,
                    command.kind,
                )?;
                base_index += command.count;
            }
            _ => unreachable!("parse_diff_command only emits supported commands"),
        }
    }

    let remaining = base_lines.len().saturating_sub(base_index);
    if remaining > 0 {
        return Err(NodelistDiffError::UnappliedBaseLines { remaining });
    }

    Ok(output.join("\n"))
}

/// Parse the CRC value from a nodelist header line.
///
/// The header format is: `;A <date> -- Day number <day> : <crc>`
/// Returns `Ok(None)` if no CRC is present, `Ok(Some(crc))` if valid,
/// or an error if the CRC format is invalid.
fn parse_header_crc(header: &str) -> Result<Option<u16>, NodelistDiffError> {
    let Some(crc_part) = header.rsplit(':').next() else {
        return Ok(None);
    };

    let crc_str = crc_part.trim();
    if crc_str.is_empty() {
        return Ok(None);
    }

    crc_str
        .parse::<u16>()
        .map(Some)
        .map_err(|_| NodelistDiffError::InvalidCrcFormat {
            value: crc_str.to_string(),
        })
}

/// Calculate the CRC-16 checksum for nodelist content.
///
/// Uses the same CRC-16/ARC algorithm as traditional FTN nodelists.
fn calculate_nodelist_crc(lines: &[String]) -> u16 {
    let mut crc: u16 = 0;

    for line in lines.iter().skip(1) {
        // Skip header line
        for byte in line.bytes() {
            crc ^= u16::from(byte);
            for _ in 0..8 {
                if crc & 1 != 0 {
                    crc = (crc >> 1) ^ 0xA001;
                } else {
                    crc >>= 1;
                }
            }
        }
        // Include line terminator
        crc ^= 0x0A; // newline
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }

    crc
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DiffCommand {
    kind: char,
    count: usize,
}

fn normalized_lines(input: &str) -> Vec<String> {
    input
        .lines()
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect()
}

fn parse_diff_command(line: &str, line_number: usize) -> Result<DiffCommand, NodelistDiffError> {
    let mut chars = line.chars();
    let Some(kind) = chars.next() else {
        return Err(NodelistDiffError::EmptyCommand { line_number });
    };
    if !matches!(kind, 'A' | 'C' | 'D') {
        return Err(NodelistDiffError::UnsupportedCommand {
            line_number,
            command: line.to_string(),
        });
    }

    let count = chars.as_str();
    if count.is_empty() || !count.chars().all(|character| character.is_ascii_digit()) {
        return Err(NodelistDiffError::InvalidCommandCount {
            line_number,
            command: kind,
            count: count.to_string(),
        });
    }
    let parsed_count =
        count
            .parse::<usize>()
            .map_err(|_| NodelistDiffError::InvalidCommandCount {
                line_number,
                command: kind,
                count: count.to_string(),
            })?;
    if parsed_count == 0 {
        return Err(NodelistDiffError::InvalidCommandCount {
            line_number,
            command: kind,
            count: count.to_string(),
        });
    }

    Ok(DiffCommand {
        kind,
        count: parsed_count,
    })
}

fn ensure_base_lines(
    base_lines: &[String],
    base_index: usize,
    requested: usize,
    line_number: usize,
    command: char,
) -> Result<(), NodelistDiffError> {
    let remaining = base_lines.len().saturating_sub(base_index);
    if remaining < requested {
        return Err(NodelistDiffError::InputExhausted {
            line_number,
            command,
            requested,
            remaining,
        });
    }
    Ok(())
}

#[derive(Debug, Default)]
struct NodelistParser {
    zone: Option<u16>,
    net: Option<u16>,
    current_node: Option<u16>,
}

impl NodelistParser {
    fn parse_line(
        &mut self,
        line: &str,
        line_number: usize,
    ) -> Result<Option<FtnNodelistEntry>, FtnError> {
        let raw_entry = line.trim_end_matches('\r').trim();
        if raw_entry.is_empty() || raw_entry.starts_with(';') {
            return Ok(None);
        }

        let fields: Vec<_> = raw_entry.split(',').map(str::trim).collect();
        if fields.is_empty() {
            return Ok(None);
        }

        let keyword = fields[0].to_ascii_lowercase();
        match keyword.as_str() {
            "zone" => {
                let zone = parse_required_number(&fields, 1, line_number)?;
                self.zone = Some(zone);
                self.net = None;
                self.current_node = None;
                Ok(None)
            }
            "region" => Ok(None),
            "host" => {
                self.net = Some(parse_required_number(&fields, 1, line_number)?);
                self.current_node = None;
                Ok(None)
            }
            "point" => {
                let point = parse_required_number(&fields, 1, line_number)?;
                let zone = self.zone.ok_or_else(|| {
                    FtnError::Parse(format!(
                        "nodelist line {line_number} has point without zone"
                    ))
                })?;
                let net = self.net.ok_or_else(|| {
                    FtnError::Parse(format!("nodelist line {line_number} has point without net"))
                })?;
                let node = self.current_node.ok_or_else(|| {
                    FtnError::Parse(format!(
                        "nodelist line {line_number} has point without boss node"
                    ))
                })?;
                Ok(Some(FtnNodelistEntry {
                    address: FtnAddress {
                        zone,
                        net,
                        node,
                        point: Some(point),
                    },
                    name: normalized_name(&fields, 2),
                    location: normalized_name(&fields, 3),
                    sysop_name: normalized_name(&fields, 4),
                    phone: optional_field(&fields, 5),
                    speed: optional_field(&fields, 6),
                    flags: parse_flags(&fields, 7),
                    raw_entry: raw_entry.to_string(),
                }))
            }
            "pvt" | "hold" | "down" | "hub" | "boss" => {
                self.parse_node_entry(&fields, 1, 2, raw_entry, line_number)
            }
            _ => {
                if fields[0].parse::<u16>().is_ok() {
                    self.parse_node_entry(&fields, 0, 1, raw_entry, line_number)
                } else if fields[0].is_empty()
                    && fields
                        .get(1)
                        .is_some_and(|field| field.parse::<u16>().is_ok())
                {
                    self.parse_node_entry(&fields, 1, 2, raw_entry, line_number)
                } else {
                    Ok(None)
                }
            }
        }
    }

    fn parse_node_entry(
        &mut self,
        fields: &[&str],
        node_index: usize,
        name_index: usize,
        raw_entry: &str,
        line_number: usize,
    ) -> Result<Option<FtnNodelistEntry>, FtnError> {
        let zone = self.zone.ok_or_else(|| {
            FtnError::Parse(format!("nodelist line {line_number} has node without zone"))
        })?;
        let net = self.net.ok_or_else(|| {
            FtnError::Parse(format!(
                "nodelist line {line_number} has node without host net"
            ))
        })?;
        let node = parse_required_number(fields, node_index, line_number)?;
        self.current_node = Some(node);

        Ok(Some(FtnNodelistEntry {
            address: FtnAddress {
                zone,
                net,
                node,
                point: None,
            },
            name: normalized_name(fields, name_index),
            location: normalized_name(fields, name_index + 1),
            sysop_name: normalized_name(fields, name_index + 2),
            phone: optional_field(fields, name_index + 3),
            speed: optional_field(fields, name_index + 4),
            flags: parse_flags(fields, name_index + 5),
            raw_entry: raw_entry.to_string(),
        }))
    }
}

fn parse_required_number(
    fields: &[&str],
    field_index: usize,
    line_number: usize,
) -> Result<u16, FtnError> {
    let raw = fields.get(field_index).ok_or_else(|| {
        FtnError::Parse(format!(
            "nodelist line {line_number} is missing field {field_index}"
        ))
    })?;
    let value = raw.parse::<u16>().map_err(|_| {
        FtnError::Parse(format!(
            "nodelist line {line_number} has invalid numeric field {field_index}: {raw}"
        ))
    })?;
    if value == 0 {
        return Err(FtnError::Parse(format!(
            "nodelist line {line_number} uses zero where a positive address part is required"
        )));
    }
    Ok(value)
}

fn normalized_name(fields: &[&str], field_index: usize) -> Option<String> {
    fields.get(field_index).and_then(|raw| {
        let name = raw.replace('_', " ");
        let trimmed = name.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn optional_field(fields: &[&str], field_index: usize) -> Option<String> {
    fields.get(field_index).and_then(|raw| {
        let trimmed = raw.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn parse_flags(fields: &[&str], start_index: usize) -> Vec<String> {
    fields
        .iter()
        .skip(start_index)
        .filter(|field| !field.trim().is_empty())
        .map(|field| field.trim().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_host_node_and_point_rows() {
        let entries = parse_nodelist(
            "\
Zone,1,FidoNet,Somewhere,Sysop,000-0000,300
Host,105,Some_Net,Somewhere,Sysop,000-0000,300
,42,Test_BBS,City,Sysop,555-1212,9600
Point,7,Point_Node,City,Sysop,555-1212,9600
",
        )
        .expect("parse nodelist");

        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].address,
            FtnAddress {
                zone: 1,
                net: 105,
                node: 42,
                point: None
            }
        );
        assert_eq!(entries[0].name.as_deref(), Some("Test BBS"));
        assert_eq!(
            entries[1].address,
            FtnAddress {
                zone: 1,
                net: 105,
                node: 42,
                point: Some(7)
            }
        );
    }

    #[test]
    fn parses_flagged_node_rows() {
        let entries = parse_nodelist(
            "\
Zone,2,Example,City,Sysop,000-0000,300
Host,500,Example_Net,City,Sysop,000-0000,300
Pvt,10,Private_Node,City,Sysop,000-0000,300
Down,11,Down_Node,City,Sysop,000-0000,300
",
        )
        .expect("parse nodelist");

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].address.to_string(), "2:500/10");
        assert_eq!(entries[1].address.to_string(), "2:500/11");
    }

    #[test]
    fn rejects_node_without_host_context() {
        let error = parse_nodelist("Zone,1,FidoNet\n,42,Missing_Net")
            .expect_err("missing net context should fail");
        assert!(error.to_string().contains("node without host net"));
    }

    #[test]
    fn applies_nodediff_count_commands() {
        let base = "\
;A Friday, July 25, 1986 -- Day number 206 : 27712
;A
Zone,1,FidoNet,Somewhere,Sysop,000-0000,300
Host,105,Some_Net,Somewhere,Sysop,000-0000,300
,42,Old_Node,City,Sysop,555-1212,9600
,43,Keep_Node,City,Sysop,555-1212,9600
";
        let diff = "\
;A Friday, July 25, 1986 -- Day number 206 : 27712
D1
A1
;A Friday, August 1, 1986 -- Day number 213 : 05060
C3
D1
A1
,42,New_Node,City,Sysop,555-1212,9600
C1
";

        let updated = apply_nodelist_diff(base, diff).expect("apply diff");

        assert_eq!(
            updated,
            "\
;A Friday, August 1, 1986 -- Day number 213 : 05060
;A
Zone,1,FidoNet,Somewhere,Sysop,000-0000,300
Host,105,Some_Net,Somewhere,Sysop,000-0000,300
,42,New_Node,City,Sysop,555-1212,9600
,43,Keep_Node,City,Sysop,555-1212,9600"
        );
        let entries = parse_nodelist(&updated).expect("parse updated nodelist");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name.as_deref(), Some("New Node"));
        assert_eq!(entries[1].name.as_deref(), Some("Keep Node"));
    }

    #[test]
    fn rejects_nodediff_header_mismatch() {
        let error = apply_nodelist_diff(
            ";A Friday, July 25, 1986 -- Day number 206 : 27712\n",
            ";A Friday, August 1, 1986 -- Day number 213 : 05060\nD1\n",
        )
        .expect_err("header mismatch should fail");

        assert!(matches!(error, NodelistDiffError::HeaderMismatch { .. }));
    }

    #[test]
    fn rejects_unsupported_nodediff_command() {
        let error = apply_nodelist_diff(
            ";A Friday, July 25, 1986 -- Day number 206 : 27712\n",
            ";A Friday, July 25, 1986 -- Day number 206 : 27712\nX1\n",
        )
        .expect_err("unsupported command should fail");

        assert_eq!(
            error,
            NodelistDiffError::UnsupportedCommand {
                line_number: 2,
                command: "X1".to_string()
            }
        );
    }
}
