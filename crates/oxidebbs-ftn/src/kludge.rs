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

pub fn parse_kludge(_line: &str) -> Option<EchomailKludge> {
    None
}

pub fn compose_kludge(_kludge: &EchomailKludge) -> String {
    String::new()
}
