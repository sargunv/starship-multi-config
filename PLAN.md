# starship-multi-config — Implementation Plan

## Files to Create

```
starship-multi-config/
├── Cargo.toml
├── src/
│   └── main.rs
├── DESIGN.md        (done)
├── PLAN.md          (this file)
└── .gitignore
```

The entire implementation lives in a single `src/main.rs` file. The program is
small enough that splitting into modules would add complexity without benefit.

---

## Step 1: Project Scaffolding

Create `Cargo.toml` and `.gitignore`.

**Cargo.toml dependencies:**
- `toml` — TOML parse + serialize via `toml::Value` / `toml::Table`

That's the only dependency.

**Cargo.toml profile.release:**
- `strip = true` — strip debug symbols for smaller binary
- `lto = true` — link-time optimization for smaller/faster binary

**.gitignore:**
- `/target`

---

## Step 2: Implement `main.rs` — Core Logic

The `main()` function follows a linear pipeline:

### 2a. Read environment

```
STARSHIP_CONFIG → Option<String>
STARSHIP        → Option<String>
```

### 2b. Decide path: passthrough vs merge

- If `STARSHIP_CONFIG` is **unset or empty**: exec starship with no
  modifications (let it use its default).
- If `STARSHIP_CONFIG` contains **no `:`**: exec starship with
  `STARSHIP_CONFIG` set to that single path (fast path, no merge).
- If `STARSHIP_CONFIG` contains **`:`**: split into paths, proceed to merge.

### 2c. Resolve and validate source paths

- Expand `~` to `$HOME` at the start of each path (starship supports this, so
  we should too).
- Canonicalize isn't necessary — just check each path exists and is readable.
- Collect mtimes (`fs::metadata(path)?.modified()?`) for each file.

### 2d. Check cache validity

- Compute cache key: hex-encode a simple hash of the joined absolute paths.
  Use `std::hash::DefaultHasher` — we don't need cryptographic hashing, just a
  stable filename.
- Determine cache dir: `$XDG_CACHE_HOME/starship-multi-config/` or
  `~/.cache/starship-multi-config/`.
- Cache file: `{cache_dir}/{hash}.toml`
- Meta file: `{cache_dir}/{hash}.meta`
- Read meta file. It stores one line per source file:
  `{mtime_secs}.{mtime_nanos} {path}`
- Compare stored mtimes against current mtimes. If all match **and** the cached
  `.toml` file exists, skip to step 2f.

### 2e. Merge and write cache

- For each source path, read file contents and parse with
  `contents.parse::<toml::Table>()`.
- Deep-merge all tables in order (left to right = base to override).
- Serialize merged table with `toml::to_string(&merged)`.
- Create cache dir if needed (`fs::create_dir_all`).
- Write merged TOML to a temp file in the cache dir, then `fs::rename` to the
  final path (atomic on same filesystem).
- Write the meta file with current mtimes.

### 2f. Exec starship

- Find starship binary:
  1. `$STARSHIP` env var if set
  2. Search `PATH` for `starship` (using `which`-style lookup, or just rely on
     the OS by passing the bare name to exec — `Command` will search `PATH`)
- Set `STARSHIP_CONFIG` to the cached merged file path.
- Use `std::os::unix::process::CommandExt::exec()` to replace the process
  (no fork, no child process — the wrapper vanishes from the process tree).
- Pass through all `argv[1..]` as args to starship.

---

## Step 3: Implement the Deep Merge Function

A single recursive function:

```rust
fn merge(base: &mut toml::Table, override_: &toml::Table) {
    for (key, override_val) in override_ {
        match (base.get_mut(key), override_val) {
            (Some(toml::Value::Table(base_t)), toml::Value::Table(override_t)) => {
                merge(base_t, override_t);
            }
            _ => {
                base.insert(key.clone(), override_val.clone());
            }
        }
    }
}
```

~10 lines. Tables are recursively merged; everything else (scalars, arrays,
arrays of tables) is replaced wholesale by the override.

---

## Step 4: Error Handling

Wrap main logic in a function returning `Result<(), Error>`. The `main()`
function calls it and on error prints the message to stderr with the
`starship-multi-config:` prefix, then exits with code 1.

Use `anyhow` or just `Box<dyn Error>` — given the program is ~80 lines, a
simple `Box<dyn Error>` avoids another dependency.

Actually, since we want zero unnecessary deps and error paths are simple, we can
use a small custom error approach or just `.map_err()` with string messages and
a type alias. The simplest approach: return `Result<(), Box<dyn Error>>` from a
`run()` function, print in `main()`.

---

## Step 5: Build and Test

- `cargo build --release`
- Manual test with example configs:
  - `STARSHIP_CONFIG=base.toml:overrides.toml starship-multi-config init bash`
  - Verify cached file contains correct merged output
  - Verify second run uses cache (check mtime of cache file doesn't change)
  - Verify editing a source file invalidates cache
  - Verify single-path passthrough works
  - Verify unset `STARSHIP_CONFIG` works

---

## Step 6: Git Init and Initial Commit

- `git init`
- Add all files
- Initial commit

---

## Estimated Size

- `Cargo.toml`: ~15 lines
- `src/main.rs`: ~100-120 lines
- Total code to maintain: ~1 file, ~120 lines
