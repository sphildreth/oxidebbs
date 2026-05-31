//! Telnet transport and negotiation.

pub const CRATE_NAME: &str = "oxidebbs-telnet";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_set() {
        assert_eq!(CRATE_NAME, "oxidebbs-telnet");
    }
}
