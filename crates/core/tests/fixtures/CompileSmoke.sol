// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract CompileSmoke {
    event Ping(address indexed caller, uint256 value);
    error Denied();

    function ping() external {
        emit Ping(msg.sender, 7);
    }

    function fail() external pure {
        revert Denied();
    }
}
