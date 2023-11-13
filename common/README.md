# heimdall-common

This crate is a collection of common utilities used by the Heimdall library. It is not intended to be used directly, but rather as a dependency of other Heimdall crates.

## Crate Structure

```
src
├── ether
│   ├── evm                 # symbolic evm implementation
│   │   ├── core            # core evm
│   │   └── ext             # evm extensions
│   │       └── exec
│   └── lexers              # lexers for parsing the evm
├── io                      # io utilities
├── resources               # resources used by the library
├── testing                 # testing utilities
└── utils
```
