//! Core domain types and session/menu/node logic.

pub const CRATE_NAME: &str = "oxidebbs-core";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_set() {
        assert_eq!(CRATE_NAME, "oxidebbs-core");
    }
}
