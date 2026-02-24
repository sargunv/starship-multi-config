use std::{
    collections::hash_map::DefaultHasher,
    env, fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

use clap::Parser;

/// Merge multiple Starship TOML configs and print the path to the merged file.
///
/// Usage:
///   export STARSHIP_CONFIG="$(starship-multi-config base.toml overrides.toml)"
///   eval "$(starship init zsh)"
#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Use a Starship preset as the base config layer.
    /// Runs `starship preset <NAME>` to fetch the preset TOML.
    #[arg(long)]
    preset: Option<String>,

    /// Override the path to the `starship` binary (used for resolving presets).
    #[arg(long, env = "STARSHIP")]
    starship: Option<PathBuf>,

    /// TOML config files to merge (left-to-right, later files override).
    #[arg(required_unless_present = "preset")]
    configs: Vec<PathBuf>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("starship-multi-config: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    // Resolve preset config if --preset is set
    let preset_path = cli
        .preset
        .as_deref()
        .map(|name| {
            let bin = resolve_starship_bin(cli.starship.as_deref())?;
            resolve_preset(&bin, name)
        })
        .transpose()?;

    let mut paths: Vec<PathBuf> = Vec::new();

    // Prepend the preset as the base layer (user configs override it)
    if let Some(preset) = preset_path {
        paths.push(preset);
    }

    paths.extend(cli.configs);

    if paths.is_empty() {
        return Err("no config files specified".into());
    }

    if paths.len() == 1 {
        // Single source: print its path directly
        println!("{}", paths[0].display());
        return Ok(());
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

    println!("{}", cache_file.display());
    Ok(())
}

fn resolve_starship_bin(
    override_path: Option<&Path>,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    match override_path {
        Some(p) => Ok(p.to_path_buf()),
        None => {
            let bin = env::var_os("STARSHIP").unwrap_or_else(|| "starship".into());
            which::which(&bin).map_err(|e| format!("{}: {e}", bin.to_string_lossy()).into())
        }
    }
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
