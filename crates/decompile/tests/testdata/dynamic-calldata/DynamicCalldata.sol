// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

contract ArbitraryCallVulnerable {
    function execute(address target, bytes calldata data)
        external payable returns (bytes memory)
    {
        (bool ok, bytes memory result) = target.call{value: msg.value}(data);
        require(ok, "call");
        return result;
    }
}

contract AbiInferenceMatrix {
    function dynamicTypes(
        bytes calldata payload,
        address[] calldata accounts,
        uint256[][] calldata matrix,
        string calldata note
    ) external payable returns (bytes32) {
        return keccak256(payload);
    }
}
