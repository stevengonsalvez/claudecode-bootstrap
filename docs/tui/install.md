---
title: "Install ainb"
description: "Install, update, and verify ainb on macOS and Linux."
---

`ainb` publishes prebuilt release binaries for macOS (Apple Silicon) and Linux (x86_64), plus Homebrew formula and Cargo source installs.

## Install with Homebrew

If you are on macOS or Linux with Homebrew installed:

```bash
brew install ainb
```

## Quick install (curl)

The install script detects your platform and downloads the latest release binary:

```bash
curl -fsSL https://raw.githubusercontent.com/stevengonsalvez/agents-in-a-box/main/ainb-tui/install.sh | bash
```

The script installs to `/usr/local/bin` when writable, otherwise `~/.local/bin` (ensure it is in your `PATH`).

Override the install directory or pin a version:

```bash
INSTALL_DIR=$HOME/bin VERSION=1.2.0 \
  curl -fsSL https://raw.githubusercontent.com/stevengonsalvez/agents-in-a-box/main/ainb-tui/install.sh | bash
```

## Prebuilt binaries

Release binaries with `.sha256` checksums are published on [GitHub Releases](https://github.com/stevengonsalvez/agents-in-a-box/releases):

| Platform | Target Architecture | Binary |
|----------|---------------------|--------|
| macOS (Apple Silicon) | `aarch64-apple-darwin` | `ainb` |
| Linux (x86_64) | `x86_64-unknown-linux-gnu` | `ainb` |

## Install from source (cargo)

Intel macOS and ARM64 Linux builds can be installed directly with Cargo:

```bash
cargo install --git https://github.com/stevengonsalvez/agents-in-a-box --branch main ainb
```

Or build locally from a repository clone:

```bash
git clone https://github.com/stevengonsalvez/agents-in-a-box
cd agents-in-a-box/ainb-tui
cargo build --release   # binary output at target/release/ainb
```

## Windows support

Windows is supported via WSL2 (Windows Subsystem for Linux). Open your WSL2 terminal (Ubuntu or Debian recommended) and follow the Linux instructions above.

## Verify the install

Check the version and verify that system dependencies (git, tmux, and an agent CLI) are ready:

```bash
ainb --version
ainb init --check
```

## Update

If installed with Homebrew:

```bash
brew upgrade ainb
```

If installed with the curl script, re-run the install script to fetch the latest binary:

```bash
curl -fsSL https://raw.githubusercontent.com/stevengonsalvez/agents-in-a-box/main/ainb-tui/install.sh | bash
```

## Requirements

- **git** 2.30 or newer (for worktree support)
- **tmux** 3.2 or newer (for session persistence)
- At least one supported agent CLI (`claude`, `codex`, or `copilot`)
