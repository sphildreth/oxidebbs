//! Local sysop/admin tooling.

pub const CRATE_NAME: &str = "oxidebbs-sysop";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_set() {
        assert_eq!(CRATE_NAME, "oxidebbs-sysop");
    }
}
