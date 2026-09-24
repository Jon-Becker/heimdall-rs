# `bifrost`

![image](https://github.com/Jon-Becker/heimdall-rs/assets/64037729/4f236ff0-7417-4e8d-8a09-6cb6da9325da)

Bifrost is heimdall's installer and version manager. Named after the rainbow bridge in Norse mythology, `bifrost` is the bridge between heimdall and your system.

## Installation
```bash
curl -L http://get.heimdall.rs | bash
```

## Supported platforms

`bifrost` builds heimdall from source by default, which requires a C toolchain
and the OpenSSL development headers. When installing on Linux, bifrost detects
the host package manager and installs those dependencies with it:

| Package manager | Distributions (examples)                | Packages installed                     |
| --------------- | --------------------------------------- | -------------------------------------- |
| `apt-get`       | Debian, Ubuntu                          | `libssl-dev`, `build-essential`        |
| `dnf` / `yum`   | Amazon Linux 2023/2, Fedora, RHEL, CentOS | `openssl-devel`, `gcc`, `gcc-c++`, `make` |
| `pacman`        | Arch                                    | `openssl`, `base-devel`                |
| `zypper`        | openSUSE                                | `libopenssl-devel`, `gcc`, `gcc-c++`, `make` |
| `apk`           | Alpine                                  | `openssl-dev`, `build-base`            |

Both `x86_64`/`amd64` and `aarch64`/`arm64` hosts are supported. For example, an
ARM64 Amazon Linux 2023 host uses `dnf` for its build dependencies and compiles
a native `aarch64` binary — no Debian/`apt-get` assumptions and no manual
package-path workarounds are required. Pre-compiled binaries (`--binary`) are
published for `linux-amd64`, `linux-arm64`, `macos-amd64`, and `macos-arm64`.

On macOS and other non-Linux systems bifrost assumes the toolchain and OpenSSL
are provided by the system (or, on macOS, homebrew) and does not install system
packages. If bifrost cannot detect a supported package manager it prints the
dependencies to install manually instead of guessing at one.

## Usage

To install the latest stable release:
```bash
bifrost
```

To install the latest stable release (pre-compiled):
```bash
bifrost --binary
```

To install a specific branch:
```bash
bifrost --version <branch>
```

To install a specific tag:
```bash
bifrost --version <tag>
```

To install a specific tag (pre-compiled):
```bash
bifrost --version <tag> --binary
```

To list all available versions:
```bash
bifrost --list
```

To update bifrost to the latest version:
```bash
bifrost --update
```
