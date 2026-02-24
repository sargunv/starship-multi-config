# starship-multi-config

A tiny wrapper for [Starship](https://github.com/starship/starship) that
deep-merges multiple TOML config files. Set `STARSHIP_CONFIG` to a
colon-separated list of paths, and `starship-multi-config` will merge them
(left-to-right, later files override) and hand the result to `starship`. Merged
configs are cached and auto-invalidate when any source file changes.

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

Point your shell init at `starship-multi-config` instead of `starship`. All
arguments pass through transparently:

```zsh
eval "$(starship-multi-config init zsh)"
```

Set `STARSHIP_CONFIG` to a colon-separated list of config paths:

```bash
export STARSHIP_CONFIG="$HOME/.config/starship/base.toml:$HOME/.config/starship/overrides.toml"
```

Glob patterns work too. Matches are sorted alphabetically, so you can control
merge order with numeric prefixes (e.g. `01-base.toml`, `02-theme.toml`):

```bash
export STARSHIP_CONFIG="$HOME/.config/starship/conf.d/*.toml"
```

Set `STARSHIP_PRESET` to use a [Starship preset](https://starship.rs/presets/)
as the base layer. Your config files override the preset:

```bash
export STARSHIP_PRESET="gruvbox-rainbow"
export STARSHIP_CONFIG="$HOME/.config/starship/overrides.toml"
```

## Environment variables

| Variable          | Description                                          |
| ----------------- | ---------------------------------------------------- |
| `STARSHIP_CONFIG` | Colon-separated list of TOML config paths or globs   |
| `STARSHIP_PRESET` | Starship preset name to use as the base config layer |
| `STARSHIP`        | Override the path to the `starship` binary           |
