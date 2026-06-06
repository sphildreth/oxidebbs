/// Echomail and netmail control lines carried inside FTN message bodies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EchomailKludge {
    Area(String),
    Msgid(String),
    Reply(String),
    Intl(String),
    Fmpt(u16, u16),
    Topt(u16, u16),
    Flags(String),
    SeenBy(String),
    Path(String),
    Via(String),
    Tear(String),
    Origin(String),
    Unknown(String, String),
}

/// Parsed FTN message body with control lines separated from display text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtnParsedMessage {
    pub area_tag: Option<String>,
    pub kludges: Vec<EchomailKludge>,
    pub body_lines: Vec<String>,
}

/// Address list carried by `SEEN-BY` or `PATH` lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FtnAddressList {
    pub raw: String,
}

/// Strict composer for FTN control lines.
pub struct FtnMessageComposer;

impl FtnMessageComposer {
    /// Compose a body from parsed FTN pieces.
    #[must_use]
    pub fn compose(parsed: &FtnParsedMessage) -> String {
        let mut lines = Vec::new();
        if let Some(area_tag) = &parsed.area_tag {
            lines.push(compose_kludge(&EchomailKludge::Area(area_tag.clone())));
        }
        lines.extend(parsed.kludges.iter().map(compose_kludge));
        lines.extend(parsed.body_lines.iter().cloned());
        lines.join("\r")
    }
}

/// Parse a single FTN kludge/control line.
#[must_use]
pub fn parse_kludge(line: &str) -> Option<EchomailKludge> {
    let line = line.trim_end_matches(['\r', '\n']);
    let control = line.strip_prefix('\x01').unwrap_or(line);

    if let Some(value) = control.strip_prefix("AREA:") {
        return Some(EchomailKludge::Area(value.trim().to_string()));
    }
    if let Some(value) = control.strip_prefix("MSGID:") {
        return Some(EchomailKludge::Msgid(value.trim().to_string()));
    }
    if let Some(value) = control.strip_prefix("REPLY:") {
        return Some(EchomailKludge::Reply(value.trim().to_string()));
    }
    if let Some(value) = control.strip_prefix("INTL ") {
        return Some(EchomailKludge::Intl(value.trim().to_string()));
    }
    if let Some(value) = control.strip_prefix("FMPT ") {
        return value
            .trim()
            .parse::<u16>()
            .ok()
            .map(|point| EchomailKludge::Fmpt(point, point));
    }
    if let Some(value) = control.strip_prefix("TOPT ") {
        return value
            .trim()
            .parse::<u16>()
            .ok()
            .map(|point| EchomailKludge::Topt(point, point));
    }
    if let Some(value) = control.strip_prefix("FLAGS ") {
        return Some(EchomailKludge::Flags(value.trim().to_string()));
    }
    if let Some(value) = control.strip_prefix("SEEN-BY:") {
        return Some(EchomailKludge::SeenBy(value.trim().to_string()));
    }
    if let Some(value) = control.strip_prefix("PATH:") {
        return Some(EchomailKludge::Path(value.trim().to_string()));
    }
    if let Some(value) = control.strip_prefix("Via ") {
        return Some(EchomailKludge::Via(value.trim().to_string()));
    }
    if let Some(value) = control.strip_prefix("---") {
        return Some(EchomailKludge::Tear(value.trim().to_string()));
    }
    if let Some(value) = control.strip_prefix(" * Origin:") {
        return Some(EchomailKludge::Origin(value.trim().to_string()));
    }
    if let Some((key, value)) = control.split_once(':') {
        return Some(EchomailKludge::Unknown(
            key.trim().to_string(),
            value.trim().to_string(),
        ));
    }
    None
}

/// Compose a single FTN kludge/control line.
#[must_use]
pub fn compose_kludge(kludge: &EchomailKludge) -> String {
    match kludge {
        EchomailKludge::Area(value) => format!("AREA:{value}"),
        EchomailKludge::Msgid(value) => format!("\x01MSGID: {value}"),
        EchomailKludge::Reply(value) => format!("\x01REPLY: {value}"),
        EchomailKludge::Intl(value) => format!("\x01INTL {value}"),
        EchomailKludge::Fmpt(point, _) => format!("\x01FMPT {point}"),
        EchomailKludge::Topt(point, _) => format!("\x01TOPT {point}"),
        EchomailKludge::Flags(value) => format!("\x01FLAGS {value}"),
        EchomailKludge::SeenBy(value) => format!("SEEN-BY: {value}"),
        EchomailKludge::Path(value) => format!("PATH: {value}"),
        EchomailKludge::Via(value) => format!("\x01Via {value}"),
        EchomailKludge::Tear(value) => format!("--- {value}"),
        EchomailKludge::Origin(value) => format!(" * Origin: {value}"),
        EchomailKludge::Unknown(key, value) => format!("\x01{key}: {value}"),
    }
}

/// Parse a full FTN body into kludges and display body lines.
#[must_use]
pub fn parse_message_body(body: &str) -> FtnParsedMessage {
    let mut parsed = FtnParsedMessage {
        area_tag: None,
        kludges: Vec::new(),
        body_lines: Vec::new(),
    };

    for line in body.split(['\r', '\n']).filter(|line| !line.is_empty()) {
        match parse_kludge(line) {
            Some(EchomailKludge::Area(area)) => parsed.area_tag = Some(area),
            Some(kludge) => parsed.kludges.push(kludge),
            None => parsed.body_lines.push(line.to_string()),
        }
    }

    parsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_kludges() {
        assert_eq!(
            parse_kludge("AREA:OXIDE.GENERAL"),
            Some(EchomailKludge::Area("OXIDE.GENERAL".to_string()))
        );
        assert_eq!(
            parse_kludge("\x01MSGID: 42:1/100 abc"),
            Some(EchomailKludge::Msgid("42:1/100 abc".to_string()))
        );
        assert_eq!(
            parse_kludge("\x01REPLY: 42:1/100 def"),
            Some(EchomailKludge::Reply("42:1/100 def".to_string()))
        );
        assert_eq!(
            parse_kludge("\x01INTL 42:1/1 42:1/100"),
            Some(EchomailKludge::Intl("42:1/1 42:1/100".to_string()))
        );
        assert_eq!(parse_kludge("\x01FMPT 7"), Some(EchomailKludge::Fmpt(7, 7)));
        assert_eq!(parse_kludge("\x01TOPT 8"), Some(EchomailKludge::Topt(8, 8)));
        assert_eq!(
            parse_kludge("\x01FLAGS K/S"),
            Some(EchomailKludge::Flags("K/S".to_string()))
        );
        assert_eq!(
            parse_kludge("SEEN-BY: 1/100 1/101"),
            Some(EchomailKludge::SeenBy("1/100 1/101".to_string()))
        );
        assert_eq!(
            parse_kludge("PATH: 1/100"),
            Some(EchomailKludge::Path("1/100".to_string()))
        );
        assert_eq!(
            parse_kludge("\x01Via OxideBBS"),
            Some(EchomailKludge::Via("OxideBBS".to_string()))
        );
        assert_eq!(
            parse_kludge("--- OxideBBS"),
            Some(EchomailKludge::Tear("OxideBBS".to_string()))
        );
        assert_eq!(
            parse_kludge(" * Origin: Blackboard"),
            Some(EchomailKludge::Origin("Blackboard".to_string()))
        );
    }

    #[test]
    fn compose_round_trips_msgid() {
        let kludge = EchomailKludge::Msgid("42:1/100 abc".to_string());

        assert_eq!(parse_kludge(&compose_kludge(&kludge)), Some(kludge));
    }

    #[test]
    fn parses_message_body() {
        let parsed = parse_message_body("AREA:OXIDE.GENERAL\r\x01MSGID: 42:1/100 abc\rHello");

        assert_eq!(parsed.area_tag.as_deref(), Some("OXIDE.GENERAL"));
        assert_eq!(parsed.kludges.len(), 1);
        assert_eq!(parsed.body_lines, ["Hello"]);
    }

    #[test]
    fn composer_emits_area_kludges_and_body() {
        let parsed = FtnParsedMessage {
            area_tag: Some("OXIDE.GENERAL".to_string()),
            kludges: vec![EchomailKludge::Msgid("42:1/100 abc".to_string())],
            body_lines: vec!["Hello".to_string()],
        };

        assert_eq!(
            FtnMessageComposer::compose(&parsed),
            "AREA:OXIDE.GENERAL\r\x01MSGID: 42:1/100 abc\rHello"
        );
    }
}
