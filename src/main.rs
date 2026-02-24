use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    os::unix::process::CommandExt,
    path::PathBuf,
    process::Command,
    time::SystemTime,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("starship-multi-config: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // Unset or empty: let starship use its default config
    // Single path: pass through as-is
    // Multiple paths: merge them
    let config_var = match env::var_os("STARSHIP_CONFIG") {
        None => return exec_starship(None),
        Some(v) if v.is_empty() => return exec_starship(None),
        Some(v) => v,
    };

    let paths: Vec<PathBuf> = env::split_paths(&config_var)
        .filter(|p| !p.as_os_str().is_empty())
        .map(expand_tilde::expand_tilde_owned)
        .collect::<Result<_, _>>()?;

    if paths.len() < 2 {
        return exec_starship(paths.into_iter().next());
    }

    // Collect mtime metadata for each source file to check cache freshness
    let current_meta = paths
        .iter()
        .map(|p| {
            let dur = fs::metadata(p)
                .and_then(|m| m.modified())
                .map_err(|e| path_err(p, e))?
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_err(|e| path_err(p, e))?;
            Ok(format!(
                "{}.{} {}",
                dur.as_secs(),
                dur.subsec_nanos(),
                p.display()
            ))
        })
        .collect::<Result<Vec<_>, String>>()?
        .join("\n");

    // Derive cache file paths from a hash of the input paths
    let hash = format!("{:x}", {
        let mut h = DefaultHasher::new();
        paths.hash(&mut h);
        h.finish()
    });

    let dir = dirs::cache_dir()
        .ok_or("could not determine cache directory")?
        .join("starship-multi-config");
    let cache_file = dir.join(format!("{hash}.toml"));
    let meta_file = dir.join(format!("{hash}.meta"));

    // Re-merge only if source files have changed since last cached merge
    let cache_valid =
        fs::read_to_string(&meta_file).is_ok_and(|s| s == current_meta) && cache_file.exists();

    if !cache_valid {
        let mut merged = toml::Table::new();
        for path in &paths {
            let content = fs::read_to_string(path).map_err(|e| path_err(path, e))?;
            let table = content
                .parse::<toml::Table>()
                .map_err(|e| path_err(path, e))?;
            merge(&mut merged, &table);
        }

        // Write cache atomically via temp file + rename
        fs::create_dir_all(&dir)?;
        let tmp = dir.join(format!("{hash}.tmp"));
        fs::write(&tmp, toml::to_string(&merged)?)?;
        fs::rename(&tmp, &cache_file)?;
        fs::write(&meta_file, &current_meta)?;
    }

    exec_starship(Some(cache_file))
}

fn exec_starship(config: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let bin = env::var("STARSHIP").unwrap_or_else(|_| "starship".to_string());
    let mut cmd = Command::new(&bin);
    cmd.args(env::args_os().skip(1));
    if let Some(path) = config {
        cmd.env("STARSHIP_CONFIG", path);
    }
    let err = cmd.exec();
    Err(format!("exec {bin}: {err}").into())
}

fn path_err(path: &std::path::Path, e: impl std::fmt::Display) -> String {
    format!("{}: {e}", path.display())
}

fn merge(base: &mut toml::Table, override_: &toml::Table) {
    for (key, override_val) in override_ {
        if let (Some(toml::Value::Table(b)), toml::Value::Table(o)) =
            (base.get_mut(key), override_val)
        {
            merge(b, o);
        } else {
            base.insert(key.clone(), override_val.clone());
        }
    }
}
