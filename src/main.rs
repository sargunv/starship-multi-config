use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    os::unix::process::CommandExt,
    path::PathBuf,
    process::Command,
    time::SystemTime,
};

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

fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        PathBuf::from(env::var("HOME").unwrap_or_default())
    } else if let Some(rest) = path.strip_prefix("~/") {
        PathBuf::from(env::var("HOME").unwrap_or_default()).join(rest)
    } else {
        PathBuf::from(path)
    }
}

fn cache_dir() -> PathBuf {
    env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env::var("HOME").unwrap_or_default()).join(".cache"))
        .join("starship-multi-config")
}

fn exec_starship(config: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let bin = env::var("STARSHIP").unwrap_or_else(|_| "starship".into());
    let mut cmd = Command::new(&bin);
    cmd.args(env::args_os().skip(1));
    if let Some(path) = config {
        cmd.env("STARSHIP_CONFIG", path);
    }
    let err = cmd.exec();
    Err(format!("exec {bin}: {err}").into())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config_var = env::var("STARSHIP_CONFIG").unwrap_or_default();

    if config_var.is_empty() {
        return exec_starship(None);
    }
    if !config_var.contains(':') {
        return exec_starship(Some(expand_tilde(&config_var)));
    }

    let paths: Vec<PathBuf> = config_var
        .split(':')
        .filter(|p| !p.is_empty())
        .map(expand_tilde)
        .collect();

    let current_meta = paths
        .iter()
        .map(|p| {
            let dur = fs::metadata(p)
                .and_then(|m| m.modified())
                .map_err(|e| format!("{}: {e}", p.display()))?
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_err(|e| format!("{}: {e}", p.display()))?;
            Ok(format!(
                "{}.{} {}",
                dur.as_secs(),
                dur.subsec_nanos(),
                p.display()
            ))
        })
        .collect::<Result<Vec<_>, String>>()?
        .join("\n");

    let mut hasher = DefaultHasher::new();
    for path in &paths {
        path.hash(&mut hasher);
    }
    let hash = format!("{:x}", hasher.finish());

    let dir = cache_dir();
    let cache_file = dir.join(format!("{hash}.toml"));
    let meta_file = dir.join(format!("{hash}.meta"));

    let cache_valid =
        fs::read_to_string(&meta_file).is_ok_and(|s| s == current_meta) && cache_file.exists();

    if !cache_valid {
        let mut merged = toml::Table::new();
        for path in &paths {
            let content =
                fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
            let table: toml::Table = content
                .parse()
                .map_err(|e| format!("{}: {e}", path.display()))?;
            merge(&mut merged, &table);
        }

        fs::create_dir_all(&dir)?;
        let tmp = dir.join(format!("{hash}.tmp"));
        fs::write(&tmp, toml::to_string(&merged)?)?;
        fs::rename(&tmp, &cache_file)?;
        fs::write(&meta_file, &current_meta)?;
    }

    exec_starship(Some(cache_file))
}

fn main() {
    if let Err(e) = run() {
        eprintln!("starship-multi-config: {e}");
        std::process::exit(1);
    }
}
