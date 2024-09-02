# Example: Invoking Heimdall via Python

This Python script demonstrates how to use the `heimdall` CLI tool to decompile smart contracts via Python. It provides a simple class structure to define arguments and manage the decompilation process, with support for customizing various decompilation options.

## Overview

The script utilizes the `heimdall decompile` command to decompile a target contract and retrieve its Solidity and Yul source code, along with its ABI. For ease of use, the script abstracts the command-line interface of `heimdall` into a Python class, allowing users to easily invoke the decompilation process programmatically.

### Key Components

- **DecompileArgs**: A class to define the arguments for the decompilation process, such as the target contract address, RPC URL, and other flags.
- **DecompiledContract**: A class to represent the output of the decompilation, including the Solidity/Yul source code and ABI.
- **Decompiler**: A class that abstracts the `heimdall decompile` command, handling argument parsing and command execution.
- **is_heimdall_installed**: A utility function to check if the `heimdall` CLI tool is installed on the system.

## Usage

1. **Install `heimdall`**: Ensure that the `heimdall` CLI tool is installed on your system.

   ```bash
   which heimdall

2. **Run the Script**: Execute the Python script to decompile a target contract.

   ```bash
   python main.py
   ```
