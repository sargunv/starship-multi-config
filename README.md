# starship-multi-config

A tiny wrapper for [Starship](https://github.com/starship/starship) that
deep-merges multiple TOML config files. Set `STARSHIP_CONFIG` to a
colon-separated list of paths, and `starship-multi-config` will merge them
(left-to-right, later files override) and hand the result to `starship`.

## Usage

Set `STARSHIP_CONFIG` to a colon-separated list of config paths:

```bash
export STARSHIP_CONFIG="$HOME/.config/starship/base.toml:$HOME/.config/starship/overrides.toml"
```

Glob patterns are supported, so you can use a `conf.d`-style directory:

```bash
export STARSHIP_CONFIG="$HOME/.config/starship/conf.d/*.toml"
```

Glob matches are sorted alphabetically within each segment, so you can control
merge order with numeric prefixes (e.g. `01-base.toml`, `02-theme.toml`).
Segments that match no files are silently ignored.

Point your shell init at `starship-multi-config` instead of `starship`:

```zsh
eval "$(starship-multi-config init zsh)"
```

All arguments pass through to `starship` transparently.

### Single path or unset

If `STARSHIP_PRESET` is not set, and `STARSHIP_CONFIG` contains no `:` separator
or is unset/empty, `starship-multi-config` passes through to `starship` with
zero overhead.

## Caching

Merged configs are cached so that normal prompt renders only pay for a few
`stat()` calls and an `exec()`. The cache key includes both the file paths and
their mtimes, so editing any source config automatically invalidates the cache.

Cache location: `$XDG_CACHE_HOME/starship-multi-config/` on Linux,
`~/Library/Caches/starship-multi-config/` on macOS.

## Presets

Set `STARSHIP_PRESET` to automatically use a
[Starship preset](https://starship.rs/presets/) as the base layer of your
config. Your own config files override the preset values:

```bash
export STARSHIP_PRESET="gruvbox-rainbow"
export STARSHIP_CONFIG="$HOME/.config/starship/overrides.toml"
eval "$(starship-multi-config init zsh)"
```

The preset TOML is fetched via `starship preset <name>` and cached until the
`starship` binary is updated.

## Environment variables

| Variable          | Description                                           |
| ----------------- | ----------------------------------------------------- |
| `STARSHIP_CONFIG` | Colon-separated list of TOML config paths or globs    |
| `STARSHIP_PRESET` | Starship preset name to use as the base config layer  |
| `STARSHIP`        | Override the path to the `starship` binary (optional) |

## Installation

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

Available assets:

- `starship-multi-config-x86_64-unknown-linux-gnu.tar.gz`
- `starship-multi-config-aarch64-apple-darwin.tar.gz`

### Prebuilt binaries

Download the latest release from
[GitHub Releases](https://github.com/sargunv/starship-multi-config/releases/latest)
and extract it somewhere on your `PATH`:

```bash
# macOS (Apple Silicon)
curl -fsSL https://github.com/sargunv/starship-multi-config/releases/latest/download/starship-multi-config-aarch64-apple-darwin.tar.gz \
  | tar xz -C ~/.local/bin

# Linux (x86_64)
curl -fsSL https://github.com/sargunv/starship-multi-config/releases/latest/download/starship-multi-config-x86_64-unknown-linux-gnu.tar.gz \
  | tar xz -C ~/.local/bin
```
