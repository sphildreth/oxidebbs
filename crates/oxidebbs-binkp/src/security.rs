use crate::BinkpError;

/// BinkP transport-security policy label.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinkpTransportSecurity {
    /// TLS is mandatory; plaintext is rejected.
    TlsRequired,
    /// TLS is attempted first; plaintext fallback is permitted for legacy peers.
    TlsOpportunistic,
    /// Plaintext is explicitly allowed for legacy FTN compatibility.
    PlaintextLegacy,
}

impl BinkpTransportSecurity {
    /// Parse a stable config/database transport-security label.
    ///
    /// # Errors
    ///
    /// Returns a protocol error when the label is not one of
    /// `tls_required`, `tls_opportunistic`, or `plaintext_legacy`.
    pub fn from_label(label: &str) -> Result<Self, BinkpError> {
        match label.trim().to_ascii_lowercase().as_str() {
            "tls_required" => Ok(Self::TlsRequired),
            "tls_opportunistic" => Ok(Self::TlsOpportunistic),
            "plaintext_legacy" => Ok(Self::PlaintextLegacy),
            other => Err(BinkpError::Protocol(format!(
                "unknown BinkP transport_security {other:?}"
            ))),
        }
    }

    /// Return the stable config/database label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TlsRequired => "tls_required",
            Self::TlsOpportunistic => "tls_opportunistic",
            Self::PlaintextLegacy => "plaintext_legacy",
        }
    }
}

/// Operator-visible transport preflight for a BinkP link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportSecurityPlan {
    pub transport_security: BinkpTransportSecurity,
    pub requires_tls: bool,
    pub attempts_tls: bool,
    pub allows_plaintext: bool,
    pub warning: Option<String>,
}

/// Build a transport-security preflight plan from a stable label.
///
/// This helper is intentionally policy-only. It does not create TLS sessions or
/// perform network I/O.
///
/// # Errors
///
/// Returns a protocol error when the transport-security label is unknown.
pub fn transport_security_plan(label: &str) -> Result<TransportSecurityPlan, BinkpError> {
    let transport_security = BinkpTransportSecurity::from_label(label)?;
    Ok(match transport_security {
        BinkpTransportSecurity::TlsRequired => TransportSecurityPlan {
            transport_security,
            requires_tls: true,
            attempts_tls: true,
            allows_plaintext: false,
            warning: None,
        },
        BinkpTransportSecurity::TlsOpportunistic => TransportSecurityPlan {
            transport_security,
            requires_tls: false,
            attempts_tls: true,
            allows_plaintext: true,
            warning: Some(
                "TLS will be attempted first; plaintext fallback is allowed for this legacy link"
                    .to_string(),
            ),
        },
        BinkpTransportSecurity::PlaintextLegacy => TransportSecurityPlan {
            transport_security,
            requires_tls: false,
            attempts_tls: false,
            allows_plaintext: true,
            warning: Some(
                "plaintext legacy BinkP is enabled; credentials and mail are not encrypted"
                    .to_string(),
            ),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tls_required_requires_and_attempts_tls() {
        let plan = transport_security_plan("tls_required").expect("plan");

        assert_eq!(plan.transport_security, BinkpTransportSecurity::TlsRequired);
        assert!(plan.requires_tls);
        assert!(plan.attempts_tls);
        assert!(!plan.allows_plaintext);
        assert_eq!(plan.warning, None);
    }

    #[test]
    fn tls_opportunistic_attempts_tls_and_warns_about_plaintext_fallback() {
        let plan = transport_security_plan("tls_opportunistic").expect("plan");

        assert_eq!(
            plan.transport_security,
            BinkpTransportSecurity::TlsOpportunistic
        );
        assert!(!plan.requires_tls);
        assert!(plan.attempts_tls);
        assert!(plan.allows_plaintext);
        assert!(
            plan.warning
                .as_deref()
                .is_some_and(|warning| warning.contains("plaintext fallback"))
        );
    }

    #[test]
    fn plaintext_legacy_allows_plaintext_without_tls_attempt() {
        let plan = transport_security_plan("plaintext_legacy").expect("plan");

        assert_eq!(
            plan.transport_security,
            BinkpTransportSecurity::PlaintextLegacy
        );
        assert!(!plan.requires_tls);
        assert!(!plan.attempts_tls);
        assert!(plan.allows_plaintext);
        assert!(
            plan.warning
                .as_deref()
                .is_some_and(|warning| warning.contains("not encrypted"))
        );
    }

    #[test]
    fn transport_security_labels_are_case_and_space_tolerant() {
        let plan = transport_security_plan(" TLS_REQUIRED ").expect("plan");

        assert_eq!(plan.transport_security.as_str(), "tls_required");
    }

    #[test]
    fn rejects_unknown_transport_security_label() {
        let error = transport_security_plan("cleartext").expect_err("invalid label");

        assert!(matches!(error, BinkpError::Protocol(_)));
    }
}
