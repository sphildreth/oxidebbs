use std::collections::HashSet;
use std::sync::Mutex;

use crate::BinkpError;

/// In-process guard for one active BinkP session per configured link.
#[derive(Debug, Default)]
pub struct LinkSessionRegistry {
    active_links: Mutex<HashSet<String>>,
}

impl LinkSessionRegistry {
    /// Create an empty session registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Acquire a session permit for a link key or id.
    ///
    /// The returned permit releases the link when dropped. This is intended for
    /// runtime poll/listener loops; it does not perform network I/O.
    ///
    /// # Errors
    ///
    /// Returns a protocol error for a blank link key or when the link already
    /// has an active session.
    pub fn try_acquire(
        &self,
        link: impl Into<String>,
    ) -> Result<LinkSessionPermit<'_>, BinkpError> {
        let link = link.into();
        let normalized = link.trim();
        if normalized.is_empty() {
            return Err(BinkpError::Protocol(
                "BinkP link session guard requires a nonblank link key".to_string(),
            ));
        }

        let mut active_links = self.active_links.lock().map_err(|_| {
            BinkpError::Protocol("BinkP link session guard lock is poisoned".to_string())
        })?;
        if !active_links.insert(normalized.to_string()) {
            return Err(BinkpError::Protocol(format!(
                "BinkP link {normalized:?} already has an active session"
            )));
        }

        Ok(LinkSessionPermit {
            registry: self,
            link: normalized.to_string(),
        })
    }

    /// Return true when the link currently has an acquired session permit.
    #[must_use]
    pub fn is_active(&self, link: &str) -> bool {
        self.active_links
            .lock()
            .is_ok_and(|active_links| active_links.contains(link.trim()))
    }

    /// Return the number of active link session permits.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active_links
            .lock()
            .map_or(0, |active_links| active_links.len())
    }
}

/// Active BinkP session permit for one link.
#[derive(Debug)]
pub struct LinkSessionPermit<'a> {
    registry: &'a LinkSessionRegistry,
    link: String,
}

impl LinkSessionPermit<'_> {
    /// Link key or id protected by this permit.
    #[must_use]
    pub fn link(&self) -> &str {
        &self.link
    }
}

impl Drop for LinkSessionPermit<'_> {
    fn drop(&mut self) {
        if let Ok(mut active_links) = self.registry.active_links.lock() {
            active_links.remove(&self.link);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_rejects_second_active_session_for_same_link() {
        let registry = LinkSessionRegistry::new();
        let first = registry.try_acquire("boss").expect("first permit");

        let error = registry
            .try_acquire("boss")
            .expect_err("second active permit");

        assert!(matches!(error, BinkpError::Protocol(_)));
        assert_eq!(first.link(), "boss");
        assert!(registry.is_active("boss"));
        assert_eq!(registry.active_count(), 1);
    }

    #[test]
    fn guard_releases_link_on_drop() {
        let registry = LinkSessionRegistry::new();
        {
            let _permit = registry.try_acquire("boss").expect("permit");
            assert!(registry.is_active("boss"));
        }

        assert!(!registry.is_active("boss"));
        assert_eq!(registry.active_count(), 0);
        assert!(registry.try_acquire("boss").is_ok());
    }

    #[test]
    fn guard_allows_different_links() {
        let registry = LinkSessionRegistry::new();
        let _first = registry.try_acquire("boss").expect("first permit");
        let _second = registry.try_acquire("hub").expect("second permit");

        assert!(registry.is_active("boss"));
        assert!(registry.is_active("hub"));
        assert_eq!(registry.active_count(), 2);
    }

    #[test]
    fn guard_rejects_blank_link() {
        let registry = LinkSessionRegistry::new();

        let error = registry.try_acquire("  ").expect_err("blank link");

        assert!(matches!(error, BinkpError::Protocol(_)));
    }
}
