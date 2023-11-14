#[cfg(test)]
mod test_types {
    use ethers::abi::ParamType;

    use crate::ether::evm::core::types::parse_function_parameters;

    #[test]
    fn test_simple_signature() {
        let solidity_type = "test(uint256)".to_string();
        let param_type = parse_function_parameters(&solidity_type);
        assert_eq!(param_type, Some(vec![ParamType::Uint(256)]));
    }

    #[test]
    fn test_multiple_signature() {
        let solidity_type = "test(uint256,string)".to_string();
        let param_type = parse_function_parameters(&solidity_type);
        assert_eq!(param_type, Some(vec![ParamType::Uint(256), ParamType::String]));
    }

    #[test]
    fn test_array_signature() {
        let solidity_type = "test(uint256,string[],uint256)";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![
                ParamType::Uint(256),
                ParamType::Array(Box::new(ParamType::String)),
                ParamType::Uint(256)
            ])
        );
    }

    #[test]
    fn test_array_fixed_signature() {
        let solidity_type = "test(uint256,string[2],uint256)";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![
                ParamType::Uint(256),
                ParamType::FixedArray(Box::new(ParamType::String), 2),
                ParamType::Uint(256)
            ])
        );
    }

    #[test]
    fn test_complex_signature() {
        let solidity_type =
            "test(uint256,string,(address,address,uint24,address,uint256,uint256,uint256,uint160))";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![
                ParamType::Uint(256),
                ParamType::String,
                ParamType::Tuple(vec![
                    ParamType::Address,
                    ParamType::Address,
                    ParamType::Uint(24),
                    ParamType::Address,
                    ParamType::Uint(256),
                    ParamType::Uint(256),
                    ParamType::Uint(256),
                    ParamType::Uint(160)
                ])
            ])
        );
    }

    #[test]
    fn test_tuple_signature() {
        let solidity_type =
            "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160))";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![ParamType::Tuple(vec![
                ParamType::Address,
                ParamType::Address,
                ParamType::Uint(24),
                ParamType::Address,
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Uint(160)
            ])])
        );
    }

    #[test]
    fn test_tuple_array_signature() {
        let solidity_type =
            "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160)[])";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![ParamType::Array(Box::new(ParamType::Tuple(vec![
                ParamType::Address,
                ParamType::Address,
                ParamType::Uint(24),
                ParamType::Address,
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Uint(256),
                ParamType::Uint(160)
            ])))])
        );
    }

    #[test]
    fn test_tuple_fixedarray_signature() {
        let solidity_type =
            "exactInputSingle((address,address,uint24,address,uint256,uint256,uint256,uint160)[2])";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![ParamType::FixedArray(
                Box::new(ParamType::Tuple(vec![
                    ParamType::Address,
                    ParamType::Address,
                    ParamType::Uint(24),
                    ParamType::Address,
                    ParamType::Uint(256),
                    ParamType::Uint(256),
                    ParamType::Uint(256),
                    ParamType::Uint(160)
                ])),
                2
            )])
        );
    }

    #[test]
    fn test_nested_tuple_signature() {
        let solidity_type = "exactInputSingle((address,address,uint24,address,uint256,(uint256,uint256)[],uint160))";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![ParamType::Tuple(vec![
                ParamType::Address,
                ParamType::Address,
                ParamType::Uint(24),
                ParamType::Address,
                ParamType::Uint(256),
                ParamType::Array(Box::new(ParamType::Tuple(vec![
                    ParamType::Uint(256),
                    ParamType::Uint(256)
                ]))),
                ParamType::Uint(160)
            ])])
        );
    }

    #[test]
    fn test_seaport_fulfill_advanced_order() {
        let solidity_type = "fulfillAdvancedOrder(((address,address,(uint8,address,uint256,uint256,uint256)[],(uint8,address,uint256,uint256,uint256,address)[],uint8,uint256,uint256,bytes32,uint256,bytes32,uint256),uint120,uint120,bytes,bytes),(uint256,uint8,uint256,uint256,bytes32[])[],bytes32,address)";
        let param_type = parse_function_parameters(solidity_type);
        assert_eq!(
            param_type,
            Some(vec![
                ParamType::Tuple(vec![
                    ParamType::Tuple(vec![
                        ParamType::Address,
                        ParamType::Address,
                        ParamType::Array(Box::new(ParamType::Tuple(vec![
                            ParamType::Uint(8),
                            ParamType::Address,
                            ParamType::Uint(256),
                            ParamType::Uint(256),
                            ParamType::Uint(256)
                        ]))),
                        ParamType::Array(Box::new(ParamType::Tuple(vec![
                            ParamType::Uint(8),
                            ParamType::Address,
                            ParamType::Uint(256),
                            ParamType::Uint(256),
                            ParamType::Uint(256),
                            ParamType::Address
                        ]))),
                        ParamType::Uint(8),
                        ParamType::Uint(256),
                        ParamType::Uint(256),
                        ParamType::FixedBytes(32),
                        ParamType::Uint(256),
                        ParamType::FixedBytes(32),
                        ParamType::Uint(256)
                    ]),
                    ParamType::Uint(120),
                    ParamType::Uint(120),
                    ParamType::Bytes,
                    ParamType::Bytes
                ]),
                ParamType::Array(Box::new(ParamType::Tuple(vec![
                    ParamType::Uint(256),
                    ParamType::Uint(8),
                    ParamType::Uint(256),
                    ParamType::Uint(256),
                    ParamType::Array(Box::new(ParamType::FixedBytes(32)))
                ]))),
                ParamType::FixedBytes(32),
                ParamType::Address
            ])
        );
    }
}
