use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    if let Err(e) = run() {
        eprintln!("starship-multi-config: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let bin = env::var_os("STARSHIP").unwrap_or_else(|| "starship".into());
    let bin_path = which::which(&bin).map_err(|e| format!("{}: {e}", bin.to_string_lossy()))?;

    let preset_var = env::var("STARSHIP_PRESET").ok().filter(|v| !v.is_empty());
    let config_var = env::var_os("STARSHIP_CONFIG");

    // Fast path: no preset and no config (or empty) -> let starship use its default
    if preset_var.is_none() && config_var.as_ref().is_none_or(|v| v.is_empty()) {
        return exec_starship(&bin_path, None);
    }

    // Resolve preset config if STARSHIP_PRESET is set
    let preset_path = preset_var
        .as_deref()
        .map(|name| resolve_preset(&bin_path, name))
        .transpose()?;

    // Expand globs from STARSHIP_CONFIG, sort matches within each segment, and flatten
    let mut paths: Vec<PathBuf> = match &config_var {
        Some(v) if !v.is_empty() => {
            let expanded = env::split_paths(v)
                .filter(|p| !p.as_os_str().is_empty())
                .map(expand_tilde::expand_tilde_owned)
                .collect::<Result<Vec<_>, _>>()?;
            let mut result = Vec::new();
            for p in expanded {
                let pattern = p.to_string_lossy();
                let mut matches: Vec<PathBuf> = glob::glob(&pattern)?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| path_err(e.path(), e.error()))?;
                matches.sort();
                result.extend(matches);
            }
            result
        }
        _ => vec![],
    };

    // Prepend the preset as the base layer (user configs override it)
    if let Some(preset) = preset_path {
        paths.insert(0, preset);
    }

    if paths.is_empty() {
        // No matches and no preset: preserve original STARSHIP_CONFIG and let starship handle it
        return exec_starship(&bin_path, config_var.map(PathBuf::from));
    }
    if paths.len() == 1 {
        // Single source: pass through as-is
        return exec_starship(&bin_path, paths.into_iter().next());
    }

    // Hash paths + mtimes to derive a cache key that invalidates when any source changes
    let hash = hash_key(|h| {
        for p in &paths {
            p.hash(h);
            let mtime = fs::metadata(p)
                .and_then(|m| m.modified())
                .map_err(|e| path_err(p, e))?;
            mtime.hash(h);
        }
        Ok(())
    })?;

    let cache_file = cache_dir()?.join(format!("{hash}.toml"));

    // Re-merge only if no cached file exists for this paths+mtimes combination
    if !cache_file.exists() {
        let mut merged = toml::Table::new();
        for path in &paths {
            let content = fs::read_to_string(path).map_err(|e| path_err(path, e))?;
            let table = content
                .parse::<toml::Table>()
                .map_err(|e| path_err(path, e))?;
            merge(&mut merged, &table);
        }

        write_cache(&cache_file, toml::to_string(&merged)?.as_bytes())?;
    }

    exec_starship(&bin_path, Some(cache_file))
}

fn resolve_preset(bin_path: &Path, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let bin_mtime = fs::metadata(bin_path)
        .and_then(|m| m.modified())
        .map_err(|e| path_err(bin_path, e))?;

    let hash = hash_key(|h| {
        name.hash(h);
        bin_path.hash(h);
        bin_mtime.hash(h);
        Ok(())
    })?;

    let cache_file = cache_dir()?.join(format!("preset-{hash}.toml"));

    if !cache_file.exists() {
        let output = Command::new(bin_path)
            .args(["preset", name])
            .output()
            .map_err(|e| format!("{}: {e}", bin_path.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("starship preset {name}: {}", stderr.trim()).into());
        }

        write_cache(&cache_file, &output.stdout)?;
    }

    Ok(cache_file)
}

fn cache_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(dirs::cache_dir()
        .ok_or("could not determine cache directory")?
        .join("starship-multi-config"))
}

fn write_cache(path: &Path, content: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let dir = path.parent().ok_or("cache file has no parent directory")?;
    fs::create_dir_all(dir)?;
    let tmp = tempfile::NamedTempFile::new_in(dir)?;
    fs::write(tmp.path(), content)?;
    tmp.persist(path)?;
    Ok(())
}

fn exec_starship(bin: &Path, config: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    let mut cmd = Command::new(bin);
    cmd.args(env::args_os().skip(1));
    match config {
        Some(path) => cmd.env("STARSHIP_CONFIG", path),
        None => cmd.env_remove("STARSHIP_CONFIG"),
    };
    cmd.env_remove("STARSHIP_PRESET");
    cmd.env_remove("STARSHIP");
    let err = cmd.exec();
    Err(format!("{}: {err}", bin.display()).into())
}

fn hash_key(
    f: impl FnOnce(&mut DefaultHasher) -> Result<(), Box<dyn std::error::Error>>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut h = DefaultHasher::new();
    f(&mut h)?;
    Ok(format!("{:x}", h.finish()))
}

fn path_err(path: &Path, e: impl std::fmt::Display) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn merge_toml(base: &str, override_: &str) -> String {
        let mut base = base.parse::<toml::Table>().unwrap();
        let override_ = override_.parse::<toml::Table>().unwrap();
        merge(&mut base, &override_);
        toml::to_string(&base).unwrap()
    }

    #[test]
    fn nested_table_merge_with_scalar_override() {
        let base = r#"
format = "$all"

[character]
success_symbol = "[>](bold green)"
error_symbol = "[>](bold red)"

[git_branch]
format = "[$branch]($style) "
style = "bold purple"
"#;

        let override_ = r#"
format = "$git_branch$character"

[character]
success_symbol = "[→](bold cyan)"
vimcmd_symbol = "[←](bold cyan)"

[package]
disabled = true
"#;

        let merged = merge_toml(base, override_);
        insta::assert_snapshot!(merged);
    }

    #[test]
    fn array_replacement() {
        let base = r#"
[palettes.base]
colors = ["red", "green", "blue"]
"#;

        let override_ = r#"
[palettes.base]
colors = ["cyan", "magenta"]
"#;

        let merged = merge_toml(base, override_);
        insta::assert_snapshot!(merged);
    }
}
