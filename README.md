# heimdall-rs

![splash preview](./preview.png?raw=true)

![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/jon-becker/heimdall-rs/tests.yml?label=Unit%20Tests)
![GitHub release (with filter)](https://img.shields.io/github/v/release/jon-becker/heimdall-rs?color=success&label=Latest%20Version)


## Overview

Heimdall is an advanced EVM smart contract toolkit specializing in bytecode analysis and extracting information from unverified contracts. Heimdall is written in Rust and is designed to be fast, modular, and more accurate than other existing tools.

Currently, Heimdall supports the following operations:
 * EVM Bytecode Disassembly
 * EVM Smart-Contract Control Flow Graph Generation
 * EVM Smart-Contract Decompilation
 * Smart-Contract Storage Dumping
 * Raw Transaction Calldata Decoding
 * Raw Transaction Trace Decoding

## Installation & Usage

Ensure that Rust & Cargo are installed:

```bash
curl https://sh.rustup.rs -sSf | sh
```

Heimdall's update and installation manager, `bifrost`, can be installed using the following command:

```bash
curl -L http://get.heimdall.rs | bash
```

If you want to manually install bifrost, you can download the latest release from [here](./bifrost/bifrost).

Once you have installed `bifrost`, you can use it to install Heimdall using the following command from a new terminal:

```bash
bifrost
```

After compilation, the `heimdall` command will be available to use from a new terminal. For advanced options, see the [bifrost documentation](https://jbecker.dev/r/heimdall-rs/wiki/installation).

_Having trouble? Check out the [Troubleshooting](https://jbecker.dev/r/heimdall-rs/wiki/troubleshooting) section in the wiki._

## Documentation

Documentation for all of heimdall-rs is available in the [wiki](https://jbecker.dev/r/heimdall-rs/wiki).

## Contributing

If you'd like to contribute to Heimdall or add a module, please open a pull-request with your changes, as well as detailed information on what is changed, added, or improved.

For more detailed information, see the [contributing guide](https://jbecker.dev/r/heimdall-rs/wiki/contributing).

## Issues

If you've found an issue or have a question, please open an issue [here](https://jbecker.dev/r/heimdall-rs/issues). All issues must follow their respective templates.

## Credits

Heimdall is a research-based toolkit created and maintained by [Jonathan Becker](https://jbecker.dev). A full list of our 20+ contributors can be found in the sidebar.

If interested in the research behind Heimdall, check out some of my [publications](https://jbecker.dev/research).

## Academic Citations

Heimdall has been cited in the following academic papers & theses:

- [The Incredible Shrinking Context... in a decompiler near you](https://www.arxiv.org/pdf/2409.11157) - Research Article, Lagouvardos S., Bollanos Y., Grech N., Smaragdakis Y., 2024.
- [Midas: Mining Profitable Exploits in On-Chain Smart Contracts via Feedback-Driven Fuzzing and Differential Analysis](https://doi.org/10.1145/3650212.3680321) - Research Article, Mingxi Ye, Xingwei Lin, Yuhong Nan, Jiajing Wu, and Zibin Zheng, ISSTA 2024.
- [From Bytecode to Safety - Decompiling Smart Contracts for Vulnerability Analysis](https://lnu.diva-portal.org/smash/get/diva2:1864948/FULLTEXT01.pdf) - Bachelors Thesis, Malek Darwish, Linnaeus University, 2024.
- [Extraction, indexing and analysis of Ethereum smart contracts data](https://webthesis.biblio.polito.it/28450/1/tesi.pdf) - Masters Thesis, Davide Aimar, Politecnico di Torino, 2023.

If you have used or plan to use Heimdall in your research, please reach out to me via [email](mailto:jonathan@jbecker.dev) or [Twitter](https://x.com/BeckerrJon)! I'd love to hear about what you're using heimdall for :)
