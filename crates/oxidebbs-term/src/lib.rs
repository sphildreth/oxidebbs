//! ANSI/CP437 terminal rendering helpers.

pub const CRATE_NAME: &str = "oxidebbs-term";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_set() {
        assert_eq!(CRATE_NAME, "oxidebbs-term");
    }
}
