# Example: Invoking Heimdall via TypeScript

This TypeScript script demonstrates how to use the `heimdall` CLI tool to decode calldata via TypeScript. It provides a simple class structure to define arguments and manage the decode process, with support for customizing various decode options.

_Note: This is just an example for the decode module, but a similar approach will work for all heimdall modules._

## Overview

The script utilizes the `heimdall decode` command to decode a target. For ease of use, the script abstracts the command-line interface of `heimdall` into a TS class, allowing users to easily call the decode process in their TS projects.
