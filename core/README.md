# heimdall-core

This crate is the core of the Heimdall library. It contains all module implementations, such as decompilation, disassembly, decoding, etc.

## Crate Structure

```
core
├── src
│   ├── cfg                                 # control flow graph module
│   ├── decode                              # calldata decoding module
│   ├── decompile                           # decompilation module
│   │   ├── analyzers                       # decompilation analyzers
│   │   └── out                             # decompilation output handlers
│   │       └── postprocessers
│   ├── disassemble                         # disassembly module
│   ├── dump                                # storage dump module
│   │   ├── menus                           # storage dump tui menus
│   │   ├── structures
│   │   └── util
│   │       └── threads
│   └── snapshot                            # snapshot module
│       ├── menus                           # snapshot tui menus
│       ├── structures
│       └── util
└── tests
```
