# magnusup

Official installer for [Magnus](https://magnus.xyz) - a blockchain for payments at scale.

## Quick Install

```bash
curl -L https://magnus.xyz/install | bash
```

## Usage

```bash
magnusup                  # Install latest release
magnusup -i v1.0.0        # Install specific version
magnusup -v               # Print installer version
magnusup --update         # Update magnusup itself
magnusup --help           # Show help
```

## Supported Platforms

- **Linux**: x86_64, arm64
- **macOS**: Apple Silicon (arm64)
- **Windows**: x86_64, arm64

## Installation Directory

Default: `~/.magnus/bin/`

Customize with `MAGNUS_DIR` environment variable:
```bash
MAGNUS_DIR=/custom/path magnusup
```

## Updating

### Update Magnus Binary

Simply run magnusup again:

```bash
magnusup
```

### Update Magnusup Itself

Use the built-in update command:

```bash
magnusup --update
```

This will:
1. Check the latest version available on GitHub
2. Download and replace the magnusup script if a newer version exists
3. Notify you of the version change

**Note:** Magnusup automatically checks for updates when you run it and will warn you if your version is outdated.

## Uninstalling

```bash
rm -rf ~/.magnus
```

Then remove the PATH export from your shell configuration file (`~/.zshenv`, `~/.bashrc`, `~/.config/fish/config.fish`, etc.).