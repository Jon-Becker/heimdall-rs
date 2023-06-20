#[cfg(test)]
mod tests {

    use crate::decompile::out::postprocessers::solidity::postprocess;
    use indicatif::ProgressBar;
    use std::collections::HashMap;

    #[test]
    fn test_convert_bitmask_to_casting_cast_before_expr() {
        let lines = vec![String::from(
            "(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff) & (arg0);",
        )];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint256(arg0);")]
        );
    }

    #[test]
    fn test_convert_bitmask_to_casting_cast_after_expr() {
        let lines = vec![String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff);",
        )];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint256(arg0);")]
        );
    }

    #[test]
    fn test_convert_bitmask_to_casting_cast_before_expr_unusual_size() {
        let lines = vec![String::from(
            "(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00) & (arg0);",
        )];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint248(arg0);")]
        );
    }

    #[test]
    fn test_convert_bitmask_to_casting_cast_after_expr_unusual_size() {
        let lines = vec![String::from(
            "(arg0) & (0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00);",
        )];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("uint248(arg0);")]
        );
    }

    #[test]
    fn test_convert_bitmask_to_casting_double_cast() {
        let lines = vec![String::from(
            "(0xffffffffffffffffffffffffffffffffffffffff) & ((0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff00) & (arg0));",
        )];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("address(uint248(arg0));")]
        );
    }

    #[test]
    fn test_simplify_casts() {
        let lines = vec![String::from("address(address(arg0));")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("address(arg0);")]
        );
    }

    #[test]
    fn test_simplify_casts_multiple_casts() {
        let lines = vec![String::from("address(address(uint248(arg0)));")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("address(uint248(arg0));")]
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
    fn test_simplify_parentheses_multiple_exprs() {
        let lines = vec![String::from("((arg0) + (arg1))")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("arg0 + arg1")]
        );
    }

    #[test]
    fn test_simplify_parentheses_within_condition() {
        let lines = vec![String::from("if ((cast(((arg0) + 1) / 10))) {")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (cast(arg0 + 1 / 10)) {")]
        );
    }

    #[test]
    fn test_simplify_parentheses_within_condition_many_unnecessary() {
        let lines = vec![
            String::from("if (((((((((((((((cast(((((((((((arg0 * (((((arg1))))))))))))) + 1)) / 10)))))))))))))))) {"),
        ];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("if (cast((arg0 * (arg1)) + 1 / 10)) {")]
        );
    }

    #[test]
    fn test_simplify_parentheses_within_precompile() {
        let lines = vec![String::from("ecrecover((0, 0, 0, 0, ((arg0)), 0))")];

        assert_eq!(
            postprocess(lines, HashMap::new(), HashMap::new(), &ProgressBar::new(128)),
            vec![String::from("ecrecover(0, 0, 0, 0, arg0, 0)")]
        );
    }
}
