// SPDX-License-Identifier: MIT
pragma solidity ^0.8.0;

contract R {
    function f(uint256 x) external pure returns (uint256) {
        require(x <= 8, "E0");
        return 0xffff;
    }
}
