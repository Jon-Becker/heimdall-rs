#[cfg(test)]
mod postprocess_tests {

    use crate::decompile::postprocess::postprocess;

    #[test]
    fn test_bitmask_conversion() {
        let mut lines = vec![
            String::from("(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff) & (arg0);"),
        ];

        assert_eq!(postprocess(lines), vec![String::from("uint256(arg0);")]);
    }
    
    #[test]
    fn test_bitmask_conversion_mask_after() {
        let mut lines = vec![
            String::from("(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff);"),
        ];

        assert_eq!(postprocess(lines), vec![String::from("uint256(arg0);")]);
    }

    #[test]
    fn test_bitmask_conversion_unusual_mask() {
        let mut lines = vec![
            String::from("(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00);"),
        ];

        assert_eq!(postprocess(lines), vec![String::from("uint248(arg0);")]);
    }

    #[test]
    fn test_simplify_casts() {
        let mut lines = vec![
            String::from("uint256(uint256(arg0));"),
        ];

        assert_eq!(postprocess(lines), vec![String::from("uint256(arg0);")]);
    }

    #[test]
    fn test_simplify_casts_complex() {
        let mut lines = vec![
            String::from("ecrecover(uint256(uint256(arg0)), uint256(uint256(arg0)), uint256(uint256(uint256(arg0))));"),
        ];

        assert_eq!(postprocess(lines), vec![String::from("ecrecover(uint256(arg0), uint256(arg0), uint256(arg0));")]);
    }

    #[test]
    fn test_iszero_flip() {
        let mut lines = vec![
            String::from("if (iszero(arg0)) {"),
        ];

        assert_eq!(postprocess(lines), vec![String::from("if (!arg0) {")]);
    }

    #[test]
    fn test_iszero_flip_complex() {
        let mut lines = vec![
            String::from("if (iszero(iszero(arg0))) {"),
        ];

        assert_eq!(postprocess(lines), vec![String::from("if (arg0) {")]);
    }

    #[test]
    fn test_iszero_flip_complex2() {
        let mut lines = vec![
            String::from("if (iszero(iszero(iszero(arg0)))) {"),
        ];

        assert_eq!(postprocess(lines), vec![String::from("if (!arg0) {")]);
    }

    #[test]
    fn test_simplify_parentheses() {
        let mut lines = vec![
            String::from("((arg0))"),
        ];

        assert_eq!(postprocess(lines), vec![String::from("arg0")]);
    }

    #[test]
    fn test_simplify_parentheses_complex() {
        let mut lines = vec![
            String::from("if ((cast(((arg0) + 1) / 10))) {"),
        ];

        assert_eq!(postprocess(lines), vec![String::from("if (cast(arg0 + 1 / 10)) {")]);
    }

    #[test]
    fn test_simplify_parentheses_complex2() {
        let mut lines = vec![
            String::from("if (((((((((((((((cast(((((((((((arg0 * (((((arg1))))))))))))) + 1)) / 10)))))))))))))))) {"),
        ];

        assert_eq!(postprocess(lines), vec![String::from("if (cast((arg0 * (arg1)) + 1 / 10)) {")]);
    }
}