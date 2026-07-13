#[cfg(test)]
mod tests {
    use oxidebbs_core::*;

    #[test]
    fn constants_have_expected_values() {
        assert_eq!(DEFAULT_TIME_LIMIT_MINUTES, 30);
        assert_eq!(DEFAULT_BINKP_PORT, 24554);
    }
}
