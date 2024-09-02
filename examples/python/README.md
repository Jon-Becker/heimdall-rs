# Example: Invoking Heimdall via Python

This Python script demonstrates how to use the `heimdall` CLI tool to decompile smart contracts via Python. It provides a simple class structure to define arguments and manage the decompilation process, with support for customizing various decompilation options.

_Note: This is just an example for the decompile module, but a similar approach will work for all heimdall modules._

## Overview

The script utilizes the `heimdall decompile` command to decompile a target contract and retrieve its Solidity and Yul source code, along with its ABI. For ease of use, the script abstracts the command-line interface of `heimdall` into a Python class, allowing users to easily call the decompilation process in their Python scripts.
