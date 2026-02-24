# starship-multi-config

A tiny tool for [Starship](https://github.com/starship/starship) that
deep-merges multiple TOML config files and prints the path to the merged result.
Set `STARSHIP_CONFIG` to its output and use `starship` as normal. Merged configs
are cached and auto-invalidate when any source file changes.

## Installation

### Prebuilt binaries

Download from
[GitHub Releases](https://github.com/sargunv/starship-multi-config/releases/latest)
and place on your `PATH`.

### [mise](https://mise.jdx.dev)

```bash
mise use -g "github:sargunv/starship-multi-config"
```

### [chezmoi](https://www.chezmoi.io)

Add to your `.chezmoiexternal.toml`, adjusting the asset name for your platform:

```toml
[".local/bin/starship-multi-config"]
type = "archive-file"
url = {{ gitHubLatestReleaseAssetURL "sargunv/starship-multi-config" "starship-multi-config-aarch64-apple-darwin.tar.gz" | quote }}
executable = true
path = "starship-multi-config"
```

## Usage

`starship-multi-config` takes config file paths as arguments, merges them
left-to-right (later files override earlier ones), and prints the path to the
merged config file. Use it to set `STARSHIP_CONFIG` before initializing
Starship:

```zsh
export STARSHIP_CONFIG="$(starship-multi-config ~/.config/starship/config.toml ~/.config/starship/conf.d/*.toml)"
eval "$(starship init zsh)"
```

Use `--preset` to apply a [Starship preset](https://starship.rs/presets/) as the
base layer. Your config files override the preset:

```zsh
export STARSHIP_CONFIG="$(starship-multi-config --preset gruvbox-rainbow ~/.config/starship.toml)"
eval "$(starship init zsh)"
```

## CLI reference

```
starship-multi-config [OPTIONS] [CONFIGS]...
```

### Arguments

| Argument     | Description                                                |
| ------------ | ---------------------------------------------------------- |
| `[CONFIGS]…` | TOML config files to merge (left-to-right, later override) |

### Options

| Option            | Description                                          |
| ----------------- | ---------------------------------------------------- |
| `--preset <NAME>` | Starship preset name to use as the base config layer |
| `-h, --help`      | Print help                                           |
| `-V, --version`   | Print version                                        |
