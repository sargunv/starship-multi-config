# starship-multi-config — Design

## Problem

[Starship](https://github.com/starship/starship) reads a single TOML config
file via the `STARSHIP_CONFIG` env var. There is no built-in way to layer or
compose configs — e.g. a base theme + machine-specific overrides.

## Solution

A tiny Rust CLI wrapper that:

1. Reads `STARSHIP_CONFIG` as a `:`-separated list of paths (like `PATH`)
2. Deep-merges those TOML files (later files override earlier ones)
3. Caches the merged result (keyed on source file mtimes)
4. Execs the real `starship` binary with `STARSHIP_CONFIG` pointing at the
   cached merged file

All CLI args pass through transparently. The wrapper is invisible to starship
and to the user's shell.

## Language & Dependencies

**Rust** with the [`toml`](https://crates.io/crates/toml) crate.

- `toml::Value` is a dynamic enum — parse arbitrary TOML, manipulate the tree,
  serialize back. No schema or struct definitions needed.
- `toml::Table` is `Map<String, Value>` (insertion-ordered).
- The crate handles parse + serialize; we only write the merge logic.
- Single static binary, ~1-2MB stripped. Sub-millisecond startup.
- All dependencies are compiled in. Zero runtime deps except `starship` itself.

## Merge Semantics

Recursive table merge, later file wins:

| Value type | Behavior |
|---|---|
| **Table** (including nested module configs like `[git_branch]`, `[aws.region_aliases]`, `[palettes.catppuccin]`) | Recursively merge keys. Keys in later files override same-key in earlier files. Keys only in earlier files are preserved. |
| **Scalar** (string, integer, float, boolean, datetime) | Last writer wins. |
| **Array** (including arrays of tables like `[[battery.display]]`) | Last writer wins (replaced entirely, not appended). |

This matches user expectations: the first file is the "base", later files are
"overrides" that surgically replace what they specify.

### Example

**base.toml:**
```toml
add_newline = true
[git_branch]
format = "[$branch]($style) "
style = "bold purple"
[aws.region_aliases]
us-east-1 = "ue1"
eu-west-1 = "ew1"
```

**overrides.toml:**
```toml
[git_branch]
style = "bold green"
[aws.region_aliases]
us-east-1 = "virginia"
```

**Merged result:**
```toml
add_newline = true
[git_branch]
format = "[$branch]($style) "
style = "bold green"
[aws.region_aliases]
us-east-1 = "virginia"
eu-west-1 = "ew1"
```

## Caching

Merging and serializing TOML on every prompt would add latency. Instead:

- **Cache location:** `$XDG_CACHE_HOME/starship-multi-config/` (defaults to
  `~/.cache/starship-multi-config/`)
- **Cache key:** A hex-encoded hash of the sorted, absolute config paths
  concatenated together. This gives a stable filename per unique set of configs.
- **Validity:** A sidecar `.meta` file stores the mtime (as seconds +
  nanoseconds since epoch) of each source file at the time of merge. On each
  invocation, stat all source files and compare. If all mtimes match, use the
  cached file. Otherwise, re-merge.
- **Cache file format:** Valid TOML, written atomically (write to temp file,
  then rename).

This means the merge only runs when a source config file actually changes.
Normal prompt renders hit a few `stat()` calls and an `exec()` — negligible.

## Finding the Real Starship

1. If `STARSHIP` env var is set, use that as the path to the starship binary.
2. Otherwise, search `PATH` for `starship`.
3. Error with a clear message if not found.

## Single-Path Fast Path

If `STARSHIP_CONFIG` contains no `:` separator (i.e. a single path), skip all
merge/cache logic entirely. Just set `STARSHIP_CONFIG` to that single path and
exec starship. Zero overhead for users who haven't opted into multi-config.

If `STARSHIP_CONFIG` is unset or empty, don't set it at all — let starship use
its default config location.

## CLI Passthrough

All arguments (`argv[1..]`) pass through to starship unmodified. The wrapper
does not define any CLI flags of its own. It communicates only through env vars:

- `STARSHIP_CONFIG` — input: colon-separated paths; output: single merged path
- `STARSHIP` — optional: explicit path to the starship binary

## Error Handling

- If any source config file doesn't exist or isn't readable: print error to
  stderr and exit 1.
- If a config file contains invalid TOML: print error to stderr and exit 1.
- If the cache directory can't be created: print error to stderr and exit 1.
- If starship binary isn't found: print error to stderr and exit 1.

All errors are prefixed with `starship-multi-config:` for easy identification.

## Binary Name

`starship-multi-config`. Users configure their shell init to call this instead
of `starship` directly:

```sh
# .zshrc / .bashrc
eval "$(starship-multi-config init zsh)"
```
