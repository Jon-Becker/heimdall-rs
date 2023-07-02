#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use indicatif::ProgressBar;

    use crate::decompile::out::postprocessers::solidity::postprocess;

    #[test]
    fn test_bitmask_conversion() {
        let lines = vec![String::from(
            "(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff) & (arg0);",
        )];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint256(arg0);")]
        );
    }

    #[test]
    fn test_bitmask_conversion_mask_after() {
        let lines = vec![String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff);",
        )];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint256(arg0);")]
        );
    }

    #[test]
    fn test_bitmask_conversion_unusual_mask() {
        let lines = vec![String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00);",
        )];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint248(arg0);")]
        );
    }

    #[test]
    fn test_simplify_casts() {
        let lines = vec![String::from("uint256(uint256(arg0));")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint256(arg0);")]
        );
    }

    #[test]
    fn test_simplify_casts_complex() {
        let lines = vec![
            String::from("ecrecover(uint256(uint256(arg0)), uint256(uint256(arg0)), uint256(uint256(uint256(arg0))));"),
        ];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("ecrecover(uint256(arg0), uint256(arg0), uint256(arg0));")]
        );
    }

    #[test]
    fn test_iszero_flip() {
        let lines = vec![String::from("if (!(arg0)) {")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (!arg0) {")]
        );
    }

    #[test]
    fn test_iszero_flip_complex() {
        let lines = vec![String::from("if (!(!(arg0))) {")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (arg0) {")]
        );
    }

    #[test]
    fn test_iszero_flip_complex2() {
        let lines = vec![String::from("if (!(!(!(arg0)))) {")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (!arg0) {")]
        );
    }

    #[test]
    fn test_simplify_parentheses() {
        let lines = vec![String::from("((arg0))")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("arg0")]
        );
    }

    #[test]
    fn test_simplify_parentheses_complex() {
        let lines = vec![String::from("if ((cast(((arg0) + 1) / 10))) {")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (cast((arg0 + 1) / 10)) {")]
        );
    }

    #[test]
    fn test_simplify_parentheses_complex2() {
        let lines = vec![
            String::from("if (((((((((((((((cast(((((((((((arg0 * (((((arg1))))))))))))) + 1)) / 10)))))))))))))))) {"),
        ];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (cast(((arg0 * (arg1)) + 1) / 10)) {")]
        );
    }
}
