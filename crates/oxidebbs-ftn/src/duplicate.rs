use oxidebbs_network::DuplicateDetectionKey;
use sha2::{Digest, Sha256};

const FALLBACK_SKEW_SECONDS: i64 = 300;

/// Duplicate detection backend for imported network messages.
pub trait DuplicateDetector {
    /// Returns true when the key has already been seen.
    fn is_duplicate(&self, key: &DuplicateDetectionKey) -> bool;
}

/// Duplicate detector that never reports duplicates.
pub struct NullDuplicateDetector;

impl DuplicateDetector for NullDuplicateDetector {
    fn is_duplicate(&self, _key: &DuplicateDetectionKey) -> bool {
        false
    }
}

/// DecentDB-backed duplicate detector using `network_duplicate_log`.
pub struct DecentDbDuplicateDetector<'a> {
    db: &'a oxidebbs_db::Db,
}

impl<'a> DecentDbDuplicateDetector<'a> {
    /// Create a duplicate detector over an open DecentDB handle.
    #[must_use]
    pub const fn new(db: &'a oxidebbs_db::Db) -> Self {
        Self { db }
    }

    /// Check duplicate state and return database errors to callers that can
    /// abort the import explicitly.
    ///
    /// # Errors
    ///
    /// Returns DecentDB errors from the duplicate-log query.
    pub fn try_is_duplicate(
        &self,
        key: &DuplicateDetectionKey,
    ) -> Result<bool, oxidebbs_db::DbError> {
        self.try_is_duplicate_any(std::slice::from_ref(key))
    }

    /// Check whether any candidate key has already been logged.
    ///
    /// # Errors
    ///
    /// Returns DecentDB errors from the duplicate-log query.
    pub fn try_is_duplicate_any(
        &self,
        keys: &[DuplicateDetectionKey],
    ) -> Result<bool, oxidebbs_db::DbError> {
        let records = oxidebbs_db::list_network_duplicates(self.db)?;
        Ok(keys.iter().any(|key| {
            records.iter().any(|record| {
                record.network_id == key.network_id
                    && record.duplicate_hash == key.message_id
                    && record.area_tag.as_deref() == Some(key.area_tag.as_str())
                    && record.origin_address == key.origin.to_string()
            })
        }))
    }
}

impl DuplicateDetector for DecentDbDuplicateDetector<'_> {
    fn is_duplicate(&self, key: &DuplicateDetectionKey) -> bool {
        self.try_is_duplicate(key).unwrap_or(true)
    }
}

/// Build an echomail duplicate key from MSGID when available, otherwise from a
/// stable fallback body hash.
#[must_use]
pub fn duplicate_key(
    network_id: impl Into<String>,
    area_tag: impl Into<String>,
    origin: oxidebbs_network::FtnAddress,
    msgid: Option<&str>,
    body: &[u8],
) -> DuplicateDetectionKey {
    echomail_duplicate_key(network_id, area_tag, origin, msgid, 0, "", body)
}

/// Build the canonical echomail duplicate key for one message.
#[must_use]
pub fn echomail_duplicate_key(
    network_id: impl Into<String>,
    area_tag: impl Into<String>,
    origin: oxidebbs_network::FtnAddress,
    msgid: Option<&str>,
    created_at_unix_secs: i64,
    subject: &str,
    body: &[u8],
) -> DuplicateDetectionKey {
    let network_id = network_id.into();
    let area_tag = area_tag.into();
    let origin_text = origin.to_string();
    let message_id = msgid
        .filter(|value| !value.trim().is_empty())
        .map(|value| sha256_hex([network_id.as_str(), area_tag.as_str(), value.trim()]))
        .unwrap_or_else(|| {
            fallback_hash(
                &network_id,
                &area_tag,
                &origin_text,
                timestamp_bucket(created_at_unix_secs),
                subject,
                body,
            )
        });

    DuplicateDetectionKey {
        network_id,
        area_tag,
        origin,
        message_id,
    }
}

/// Build fallback echomail duplicate-key candidates that tolerate +/- five
/// minutes of clock skew.
#[must_use]
pub fn echomail_fallback_duplicate_candidates(
    network_id: impl Into<String>,
    area_tag: impl Into<String>,
    origin: oxidebbs_network::FtnAddress,
    created_at_unix_secs: i64,
    subject: &str,
    body: &[u8],
) -> Vec<DuplicateDetectionKey> {
    let network_id = network_id.into();
    let area_tag = area_tag.into();
    let origin_text = origin.to_string();
    let bucket = timestamp_bucket(created_at_unix_secs);
    (bucket - 1..=bucket + 1)
        .map(|candidate_bucket| DuplicateDetectionKey {
            network_id: network_id.clone(),
            area_tag: area_tag.clone(),
            origin: origin.clone(),
            message_id: fallback_hash(
                &network_id,
                &area_tag,
                &origin_text,
                candidate_bucket,
                subject,
                body,
            ),
        })
        .collect()
}

/// Build the canonical netmail duplicate key.
#[must_use]
pub fn netmail_duplicate_key(
    network_id: impl Into<String>,
    from: oxidebbs_network::FtnAddress,
    to: oxidebbs_network::FtnAddress,
    msgid: Option<&str>,
    created_at_unix_secs: i64,
    subject: &str,
    body: &[u8],
) -> DuplicateDetectionKey {
    let network_id = network_id.into();
    let to_text = to.to_string();
    let area_tag = format!("netmail:{to_text}");
    let origin = from;
    let origin_text = origin.to_string();
    let message_id = msgid
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            sha256_hex([
                network_id.as_str(),
                origin_text.as_str(),
                to_text.as_str(),
                value.trim(),
            ])
        })
        .unwrap_or_else(|| {
            fallback_hash(
                &network_id,
                &area_tag,
                &origin_text,
                timestamp_bucket(created_at_unix_secs),
                subject,
                body,
            )
        });

    DuplicateDetectionKey {
        network_id,
        area_tag,
        origin,
        message_id,
    }
}

/// Build fallback netmail duplicate-key candidates that tolerate +/- five
/// minutes of clock skew.
#[must_use]
pub fn netmail_fallback_duplicate_candidates(
    network_id: impl Into<String>,
    from: oxidebbs_network::FtnAddress,
    to: oxidebbs_network::FtnAddress,
    created_at_unix_secs: i64,
    subject: &str,
    body: &[u8],
) -> Vec<DuplicateDetectionKey> {
    let network_id = network_id.into();
    let to_text = to.to_string();
    let area_tag = format!("netmail:{to_text}");
    let origin_text = from.to_string();
    let bucket = timestamp_bucket(created_at_unix_secs);
    (bucket - 1..=bucket + 1)
        .map(|candidate_bucket| DuplicateDetectionKey {
            network_id: network_id.clone(),
            area_tag: area_tag.clone(),
            origin: from.clone(),
            message_id: fallback_hash(
                &network_id,
                &area_tag,
                &origin_text,
                candidate_bucket,
                subject,
                body,
            ),
        })
        .collect()
}

fn fallback_hash(
    network_id: &str,
    area_or_recipient: &str,
    origin: &str,
    timestamp_bucket: i64,
    subject: &str,
    body: &[u8],
) -> String {
    let body_hash = sha256_bytes(body);
    sha256_hex([
        network_id,
        area_or_recipient,
        origin,
        &timestamp_bucket.to_string(),
        subject.trim(),
        &body_hash,
    ])
}

fn timestamp_bucket(seconds: i64) -> i64 {
    seconds.div_euclid(FALLBACK_SKEW_SECONDS)
}

fn sha256_hex<const N: usize>(parts: [&str; N]) -> String {
    let mut hasher = Sha256::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            hasher.update([0]);
        }
        hasher.update(part.as_bytes());
    }
    hex_lower(&hasher.finalize())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_lower(&hasher.finalize())
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

#[cfg(test)]
mod tests {
    use super::*;
    use oxidebbs_db::{
        NetworkDuplicateLogRecord, NetworkProfileRecord, OxideDb, insert_network_duplicate_log,
        insert_network_profile,
    };

    #[test]
    fn duplicate_key_prefers_msgid() {
        let key = duplicate_key(
            "net",
            "AREA",
            "42:1/100".parse().expect("origin"),
            Some("42:1/100 abc"),
            b"body",
        );

        let expected = sha256_hex(["net", "AREA", "42:1/100 abc"]);
        assert_eq!(key.message_id, expected);
    }

    #[test]
    fn duplicate_key_falls_back_to_stable_body_hash() {
        let origin: oxidebbs_network::FtnAddress = "42:1/100".parse().expect("origin");
        let left = duplicate_key("net", "AREA", origin.clone(), None, b"body");
        let right = duplicate_key("net", "AREA", origin, Some(" "), b"body");

        assert_eq!(left.message_id, right.message_id);
        assert_eq!(left.message_id.len(), 64);
    }

    #[test]
    fn fallback_candidates_cover_five_minute_clock_skew() {
        let origin: oxidebbs_network::FtnAddress = "42:1/100".parse().expect("origin");
        let canonical =
            echomail_duplicate_key("net", "AREA", origin.clone(), None, 600, "Subject", b"body");
        let candidates =
            echomail_fallback_duplicate_candidates("net", "AREA", origin, 899, "Subject", b"body");

        assert!(
            candidates
                .iter()
                .any(|candidate| candidate.message_id == canonical.message_id)
        );
    }

    #[test]
    fn netmail_duplicate_key_uses_destination_scope() {
        let from: oxidebbs_network::FtnAddress = "42:1/100".parse().expect("from");
        let to: oxidebbs_network::FtnAddress = "42:1/1".parse().expect("to");

        let key = netmail_duplicate_key(
            "net",
            from.clone(),
            to.clone(),
            Some("42:1/100 abc"),
            0,
            "Subject",
            b"body",
        );
        let candidates =
            netmail_fallback_duplicate_candidates("net", from, to, 0, "Subject", b"body");

        assert_eq!(key.area_tag, "netmail:42:1/1");
        assert_eq!(key.message_id.len(), 64);
        assert_eq!(candidates.len(), 3);
    }

    #[test]
    fn decentdb_detector_finds_duplicate_log_match() {
        let db = OxideDb::open_memory().expect("open db");
        let profile_id = "11111111-1111-1111-1111-111111111111";
        insert_network_profile(
            db.db(),
            &NetworkProfileRecord {
                id: profile_id.to_string(),
                key: "test".to_string(),
                name: "Test".to_string(),
                adapter: "legacy-ftn".to_string(),
                local_zone: 42,
                local_net: 1,
                local_node: 100,
                local_point: 0,
                enabled: true,
                created_at: "2026-06-04T00:00:00Z".to_string(),
                updated_at: "2026-06-04T00:00:00Z".to_string(),
            },
        )
        .expect("insert profile");
        insert_network_duplicate_log(
            db.db(),
            &NetworkDuplicateLogRecord {
                id: "22222222-2222-2222-2222-222222222222".to_string(),
                network_id: profile_id.to_string(),
                duplicate_hash: sha256_hex([profile_id, "AREA", "42:1/100 abc"]),
                msgid: Some("42:1/100 abc".to_string()),
                area_tag: Some("AREA".to_string()),
                origin_address: "42:1/100".to_string(),
                detected_at: "2026-06-04T00:00:00Z".to_string(),
                action: "rejected".to_string(),
            },
        )
        .expect("insert duplicate");

        let detector = DecentDbDuplicateDetector::new(db.db());
        let key = duplicate_key(
            profile_id,
            "AREA",
            "42:1/100".parse().expect("origin"),
            Some("42:1/100 abc"),
            b"body",
        );

        assert!(detector.try_is_duplicate(&key).expect("check duplicate"));
    }
}
