#[cfg(test)]
mod tests {

    use std::str::FromStr;

    use ethers::{prelude::U256};

    use crate::ether::evm::vm::VM;
    
    // creates a new test VM with calldata.
    fn new_test_vm(bytecode: &str) -> VM {
        VM::new(
            String::from(bytecode),
            String::from("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF"),
            String::from("0x6865696d64616c6c000000000061646472657373"),
            String::from("0x6865696d64616c6c0000000000006f726967696e"),
            String::from("0x6865696d64616c6c00000000000063616c6c6572"),
            0,
            9999999999,
            "INFO"
        )
    }

    #[test]
    fn test_stop_vm() {

        let mut vm = new_test_vm("0x00");
        vm.execute();

        assert_eq!(vm.returndata, "0x");
        assert_eq!(vm.exitcode, 0);

    }

    #[test]
    fn test_pc_out_of_range() {
        let mut vm = new_test_vm("0x");
        vm.execute();

        assert_eq!(vm.returndata, "");
        assert_eq!(vm.exitcode, 255);
    }

    #[test]
    fn test_add() {
        let mut vm = new_test_vm("0x600a600a017fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff600101");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x14").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x00").unwrap());
    }

    #[test]
    fn test_mul() {
        let mut vm = new_test_vm("0x600a600a027fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff600202");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x64").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe").unwrap());
    }

    #[test]
    fn test_sub() {
        let mut vm = new_test_vm("0x600a600a036001600003");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x00").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap());
    }

    #[test]
    fn test_div() {
        let mut vm = new_test_vm("0x600a600a046002600104");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x01").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x00").unwrap());
    }

    #[test]
    fn test_sdiv() {
        let mut vm = new_test_vm("0x600a600a057fFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF7fFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFE05");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x01").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x02").unwrap());
    }

    #[test]
    fn test_mod() {
        let mut vm = new_test_vm("0x6003600a066005601106");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x01").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x02").unwrap());
    }

    #[test]
    fn test_smod() {
        let mut vm = new_test_vm("0x6003600a077ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffd7ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff807");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x01").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0xfffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffe").unwrap());
    }

    #[test]
    fn test_addmod() {
        let mut vm = new_test_vm("0x6008600a600a08600260027fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff08");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x04").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x01").unwrap());
    }

    #[test]
    fn test_mulmod() {
        let mut vm = new_test_vm("0x6008600a600a09600c7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff09");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x04").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x01").unwrap());
    }

    #[test]
    fn test_exp() {
        let mut vm = new_test_vm("0x6002600a0a600260020a");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x64").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x04").unwrap());
    }

    #[test]
    fn test_signextend() {
        let mut vm = new_test_vm("0x60ff60000b607f60000b");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x7f").unwrap());
    }

    #[test]
    fn test_lt() {
        let mut vm = new_test_vm("0x600a600910600a600a10");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x01").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x00").unwrap());
    }

    #[test]
    fn test_gt() {
        let mut vm = new_test_vm("0x6009600a11600a600a10");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x01").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x00").unwrap());
    }

    #[test]
    fn test_slt() {
        let mut vm = new_test_vm("0x60097fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff12600a600a12");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x01").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x00").unwrap());
    }

    #[test]
    fn test_sgt() {
        let mut vm = new_test_vm("0x7fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff600913600a600a13");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x01").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x00").unwrap());
    }

    #[test]
    fn test_eq() {
        let mut vm = new_test_vm("0x600a600a14600a600514");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x01").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x00").unwrap());
    }

    #[test]
    fn test_iszero() {
        let mut vm = new_test_vm("0x600015600a15");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x01").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x00").unwrap());
    }

    #[test]
    fn test_and() {
        let mut vm = new_test_vm("0x600f600f16600060ff1600");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x0F").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x00").unwrap());
    }

    #[test]
    fn test_or() {
        let mut vm = new_test_vm("0x600f60f01760ff60ff17");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0xff").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0xff").unwrap());
    }

    #[test]
    fn test_xor() {
        let mut vm = new_test_vm("0x600f60f01860ff60ff18");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0xff").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x00").unwrap());
    }

    #[test]
    fn test_not() {
        let mut vm = new_test_vm("0x600019");
        vm.execute();

        assert_eq!(vm.stack.peek(0), U256::from_str("0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").unwrap());
    }

    #[test]
    fn test_byte() {
        let mut vm = new_test_vm("0x60ff601f1a61ff00601e1a");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0xff").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0xff").unwrap());
    }

    #[test]
    fn test_shl() {
        let mut vm = new_test_vm("0x600160011b0x7fFF0000000000000000000000000000000000000000000000000000000000000060041b");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x02").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0xF000000000000000000000000000000000000000000000000000000000000000").unwrap());
    }

    #[test]
    fn test_shr() {
        let mut vm = new_test_vm("600260011c60ff60041c");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x01").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x0f").unwrap());
    }

    #[test]
    fn test_sar() {
        let mut vm = new_test_vm("600260011d");
        vm.execute();

        assert_eq!(vm.stack.peek(0), U256::from_str("0x01").unwrap());
    }

    #[test]
    fn test_sha3() {
        let mut vm = new_test_vm("0x7fffffffff000000000000000000000000000000000000000000000000000000006000526004600020");
        vm.execute();

        assert_eq!(vm.stack.peek(0), U256::from_str("0x29045A592007D0C246EF02C2223570DA9522D0CF0F73282C79A1BC8F0BB2C238").unwrap());
    }

    #[test]
    fn test_calldataload() {
        let mut vm = new_test_vm("600035601f35");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0xFF00000000000000000000000000000000000000000000000000000000000000").unwrap());
    }

    #[test]
    fn test_calldatasize() {
        let mut vm = new_test_vm("0x36");
        vm.execute();

        assert_eq!(vm.stack.peek(0), U256::from_str("0x20").unwrap());
    }

    #[test]
    fn test_xdatacopy() {
        // returndatacopy, calldatacopy, etc share same code.
        let mut vm = new_test_vm("0x60ff6000600037");
        vm.execute();
        assert_eq!(vm.memory.read(0, 32), "FFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFF");
    }

    #[test]
    fn test_mload_mstore() {
        let mut vm = new_test_vm("0x7f00000000000000000000000000000000000000000000000000000000000000FF600052600051600151");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0xff").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0xff00").unwrap());
    }

    #[test]
    fn test_sload_sstore() {
        let mut vm = new_test_vm("0x602e600055600054600154");
        vm.execute();

        assert_eq!(vm.stack.peek(1), U256::from_str("0x2e").unwrap());
        assert_eq!(vm.stack.peek(0), U256::from_str("0x00").unwrap());
    }

    #[test]
    fn test_jump() {
        let mut vm = new_test_vm("0x60fe56");
        vm.execute();

        assert_eq!(U256::from(vm.instruction as u128), U256::from_str("0xff").unwrap());
    }

    #[test]
    fn test_jumpi() {
        let mut vm = new_test_vm("0x600160fe57");
        vm.execute();

        assert_eq!(U256::from(vm.instruction as u128), U256::from_str("0xff").unwrap());

        let mut vm = new_test_vm("0x600060fe5758");
        vm.execute();

        assert_eq!(U256::from(vm.instruction as u128), U256::from_str("0x07").unwrap());
        
        // PC test
        assert_eq!(vm.stack.peek(0), U256::from_str("0x07").unwrap());
    }

}