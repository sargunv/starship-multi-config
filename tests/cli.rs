use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use tempfile::TempDir;

fn cmd() -> assert_cmd::Command {
    assert_cmd::cargo::cargo_bin_cmd!("starship-multi-config")
}

fn write_toml(dir: &TempDir, name: &str, content: &str) -> String {
    let path = dir.path().join(name);
    fs::write(&path, content).unwrap();
    path.to_str().unwrap().to_string()
}

/// Creates a starship stub. If `preset_toml` is provided, the stub also handles
/// `preset <name>` calls by outputting the given TOML content.
fn write_stub(dir: &TempDir, preset_toml: Option<&str>) -> String {
    let path = dir.path().join("starship-stub");
    let script = match preset_toml {
        Some(content) => {
            let preset_file = dir.path().join("preset-content.toml");
            fs::write(&preset_file, content).unwrap();
            format!(
                "#!/bin/sh\nif [ \"$1\" = \"preset\" ]; then\n  cat \"{}\"\nelse\n  echo \"STARSHIP_CONFIG=$STARSHIP_CONFIG\"\nfi\n",
                preset_file.display()
            )
        }
        None => "#!/bin/sh\necho \"STARSHIP_CONFIG=$STARSHIP_CONFIG\"\n".to_string(),
    };
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn passthrough_when_unset() {
    let dir = TempDir::new().unwrap();
    let stub = write_stub(&dir, None);

    cmd()
        .env("STARSHIP", &stub)
        .env_remove("STARSHIP_CONFIG")
        .assert()
        .success()
        .stdout("STARSHIP_CONFIG=\n");
}

#[test]
fn passthrough_with_single_path() {
    let dir = TempDir::new().unwrap();
    let stub = write_stub(&dir, None);

    cmd()
        .env("STARSHIP", &stub)
        .env("STARSHIP_CONFIG", "/some/path.toml")
        .assert()
        .success()
        .stdout("STARSHIP_CONFIG=/some/path.toml\n");
}

#[test]
fn merge_two_files() {
    let dir = TempDir::new().unwrap();
    let stub = write_stub(&dir, None);

    let f1 = write_toml(
        &dir,
        "base.toml",
        r#"
format = "$all"

[character]
success_symbol = "[>](bold green)"
error_symbol = "[>](bold red)"
"#,
    );

    let f2 = write_toml(
        &dir,
        "override.toml",
        r#"
[character]
success_symbol = "[→](bold cyan)"

[package]
disabled = true
"#,
    );

    let config_var = format!("{f1}:{f2}");

    let output = cmd()
        .env("STARSHIP", &stub)
        .env("STARSHIP_CONFIG", &config_var)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let cache_path = stdout.trim().strip_prefix("STARSHIP_CONFIG=").unwrap();

    assert!(
        Path::new(cache_path).exists(),
        "cache file should exist at {cache_path}"
    );

    let cached_toml = fs::read_to_string(cache_path).unwrap();
    insta::assert_snapshot!(cached_toml);
}

#[test]
fn glob_expansion() {
    let dir = TempDir::new().unwrap();
    let stub = write_stub(&dir, None);

    let conf_dir = dir.path().join("conf.d");
    fs::create_dir(&conf_dir).unwrap();

    fs::write(
        conf_dir.join("01-base.toml"),
        r#"
format = "$all"

[character]
success_symbol = "[>](bold green)"
error_symbol = "[>](bold red)"
"#,
    )
    .unwrap();

    fs::write(
        conf_dir.join("02-override.toml"),
        r#"
[character]
success_symbol = "[→](bold cyan)"

[package]
disabled = true
"#,
    )
    .unwrap();

    let config_var = format!("{}/*.toml", conf_dir.display());

    let output = cmd()
        .env("STARSHIP", &stub)
        .env("STARSHIP_CONFIG", &config_var)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let cache_path = stdout.trim().strip_prefix("STARSHIP_CONFIG=").unwrap();

    assert!(
        Path::new(cache_path).exists(),
        "cache file should exist at {cache_path}"
    );

    let cached_toml = fs::read_to_string(cache_path).unwrap();
    insta::assert_snapshot!(cached_toml);
}

#[test]
fn glob_no_match_passthrough() {
    let dir = TempDir::new().unwrap();
    let stub = write_stub(&dir, None);

    // Glob that matches nothing — original value is preserved for starship to handle
    let config_var = format!("{}/nonexistent/*.toml", dir.path().display());

    cmd()
        .env("STARSHIP", &stub)
        .env("STARSHIP_CONFIG", &config_var)
        .assert()
        .success()
        .stdout(format!("STARSHIP_CONFIG={config_var}\n"));
}

#[test]
fn invalid_glob_error() {
    let dir = TempDir::new().unwrap();
    let stub = write_stub(&dir, None);
    let good = write_toml(&dir, "good.toml", "key = 1\n");
    // Unclosed bracket is invalid glob syntax
    let config_var = format!("{good}:[unclosed");

    cmd()
        .env("STARSHIP", &stub)
        .env("STARSHIP_CONFIG", &config_var)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("Pattern syntax error"));
}

#[test]
fn invalid_toml_error() {
    let dir = TempDir::new().unwrap();
    let stub = write_stub(&dir, None);
    let good = write_toml(&dir, "good.toml", "key = 1\n");
    let bad = write_toml(&dir, "bad.toml", "this is not valid [[[ toml");
    let config_var = format!("{good}:{bad}");

    cmd()
        .env("STARSHIP", &stub)
        .env("STARSHIP_CONFIG", &config_var)
        .assert()
        .code(1)
        .stderr(predicates::str::contains("bad.toml"));
}

#[test]
fn preset_only() {
    let dir = TempDir::new().unwrap();
    let stub = write_stub(
        &dir,
        Some(
            r#"
format = "$all"

[character]
success_symbol = "[→](bold cyan)"
"#,
        ),
    );

    let output = cmd()
        .env("STARSHIP", &stub)
        .env_remove("STARSHIP_CONFIG")
        .env("STARSHIP_PRESET", "test-preset")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let cache_path = stdout.trim().strip_prefix("STARSHIP_CONFIG=").unwrap();

    // Single source (just the preset) passes through directly
    let cached_toml = fs::read_to_string(cache_path).unwrap();
    insta::assert_snapshot!(cached_toml);
}

#[test]
fn preset_with_user_config() {
    let dir = TempDir::new().unwrap();
    let stub = write_stub(
        &dir,
        Some(
            r#"
format = "$all"

[character]
success_symbol = "[→](bold cyan)"
error_symbol = "[→](bold red)"

[git_branch]
format = "[$branch]($style) "
"#,
        ),
    );

    let user_config = write_toml(
        &dir,
        "user.toml",
        r#"
[character]
success_symbol = "[>](bold green)"

[package]
disabled = true
"#,
    );

    let output = cmd()
        .env("STARSHIP", &stub)
        .env("STARSHIP_CONFIG", &user_config)
        .env("STARSHIP_PRESET", "test-preset")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let cache_path = stdout.trim().strip_prefix("STARSHIP_CONFIG=").unwrap();

    // Merged: preset is the base, user config overrides
    let cached_toml = fs::read_to_string(cache_path).unwrap();
    insta::assert_snapshot!(cached_toml);
}
