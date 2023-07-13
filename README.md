# heimdall-rs

![splash preview](./preview.png?raw=true)

![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/jon-becker/heimdall-rs/tests.yml?label=Unit%20Tests)
![GitHub release (with filter)](https://img.shields.io/github/v/release/jon-becker/heimdall-rs?color=success&label=Latest%20Version)


## Overview

Heimdall is an advanced EVM smart contract toolkit specializing in bytecode analysis. Heimdall is written in Rust and is designed to be fast, modular, and more accurate than other existing tools.

Currently, Heimdall supports the following operations:
 * EVM Bytecode Disassembly
 * EVM Smart-Contract Control Flow Graph Generation
 * EVM Smart-Contract Decompilation
 * Smart-Contract Storage Dumping
 * Transaction Calldata Decoding

## Installation & Usage

Ensure that Rust & Cargo are installed:

```bash
curl https://sh.rustup.rs -sSf | sh
```

Heimdall's update and installation manager, `bifrost`, can be installed using the following command:

```bash
curl -L http://get.heimdall.rs | bash
```

<!-- Alternatively, you can install through GitHub directly using the following command:

```bash
curl -L https://raw.githubusercontent.com/Jon-Becker/heimdall-rs/main/bifrost/install | bash
``` -->

If you want to manually install bifrost, you can download the latest release from [here](./bifrost/bifrost).

Once you have installed `bifrost`, you can use it to install Heimdall using the following command from a new terminal:

```bash
bifrost
```

After compilation, the `heimdall` command will be available to use from a new terminal. For advanced options, see the [bifrost documentation](https://jbecker.dev/r/heimdall-rs/wiki/installation).

_Having trouble? Check out the [Troubleshooting](https://jbecker.dev/r/heimdall-rs/wiki/troubleshooting) section in the wiki._

## Documentation

Documentation for all of heimdall-rs is available in the [wiki](https://jbecker.dev/r/heimdall-rs/wiki).

## Examples

Examples for heimdall-rs modules are available in the [wiki](https://jbecker.dev/r/heimdall-rs/wiki/examples).

## Contributing

If you'd like to contribute to Heimdall or add a module, please open a pull-request with your changes, as well as detailed information on what is changed, added, or improved.

For more detailed information, see the [contributing guide](https://jbecker.dev/r/heimdall-rs/wiki/contributing).

## Issues

If you've found an issue or have a question, please open an issue [here](https://jbecker.dev/r/heimdall-rs/issues). All issues must follow their respective templates.

## Credits

- Jonathan Becker \<<jonathan@jbecker.dev>>

A list of all [contributors](https://jbecker.dev/r/heimdall-rs/wiki/contributors) can be found in the wiki.
