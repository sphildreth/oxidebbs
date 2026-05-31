//! Door definitions, drop files, and runners.

pub const CRATE_NAME: &str = "oxidebbs-door";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_is_set() {
        assert_eq!(CRATE_NAME, "oxidebbs-door");
    }
}
