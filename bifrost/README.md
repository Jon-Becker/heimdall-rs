# `bifrost`

![image](https://github.com/Jon-Becker/heimdall-rs/assets/64037729/4f236ff0-7417-4e8d-8a09-6cb6da9325da)

Bifrost is heimdall's installer and version manager. Named after the rainbow bridge in Norse mythology, `bifrost` is the bridge between heimdall and your system.

## Installation
```bash
curl -L http://get.heimdall.rs | bash
```

## Usage

To install the latest stable release:
```bash
bifrost
```

To install the lastest stable release (pre-compiled):
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
