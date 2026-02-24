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

/// Creates a starship stub that handles `preset <name>` calls by outputting
/// the given TOML content.
fn write_starship_stub(dir: &TempDir, preset_toml: &str) -> String {
    let path = dir.path().join("starship-stub");
    let preset_file = dir.path().join("preset-content.toml");
    fs::write(&preset_file, preset_toml).unwrap();
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = \"preset\" ]; then\n  cat \"{}\"\nelse\n  echo \"unexpected args: $@\" >&2\n  exit 1\nfi\n",
        preset_file.display()
    );
    fs::write(&path, script).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path.to_str().unwrap().to_string()
}

#[test]
fn single_config_passthrough() {
    let dir = TempDir::new().unwrap();
    let f1 = write_toml(&dir, "base.toml", "format = \"$all\"\n");

    cmd().arg(&f1).assert().success().stdout(format!("{f1}\n"));
}

#[test]
fn merge_two_files() {
    let dir = TempDir::new().unwrap();

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

    let output = cmd()
        .args([&f1, &f2])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let cache_path = stdout.trim();

    assert!(
        Path::new(cache_path).exists(),
        "cache file should exist at {cache_path}"
    );

    let cached_toml = fs::read_to_string(cache_path).unwrap();
    insta::assert_snapshot!(cached_toml);
}

#[test]
fn no_args_shows_error() {
    cmd()
        .assert()
        .failure()
        .stderr(predicates::str::contains("Usage"));
}

#[test]
fn nonexistent_file_error() {
    cmd()
        .args(["/nonexistent/a.toml", "/nonexistent/b.toml"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("/nonexistent/a.toml"));
}

#[test]
fn invalid_toml_error() {
    let dir = TempDir::new().unwrap();
    let good = write_toml(&dir, "good.toml", "key = 1\n");
    let bad = write_toml(&dir, "bad.toml", "this is not valid [[[ toml");

    cmd()
        .args([&good, &bad])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("bad.toml"));
}

#[test]
fn preset_only() {
    let dir = TempDir::new().unwrap();
    let stub = write_starship_stub(
        &dir,
        r#"
format = "$all"

[character]
success_symbol = "[→](bold cyan)"
"#,
    );

    let output = cmd()
        .args(["--preset", "test-preset", "--starship", &stub])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let cache_path = stdout.trim();

    // Single source (just the preset) passes through directly
    let cached_toml = fs::read_to_string(cache_path).unwrap();
    insta::assert_snapshot!(cached_toml);
}

#[test]
fn preset_with_user_config() {
    let dir = TempDir::new().unwrap();
    let stub = write_starship_stub(
        &dir,
        r#"
format = "$all"

[character]
success_symbol = "[→](bold cyan)"
error_symbol = "[→](bold red)"

[git_branch]
format = "[$branch]($style) "
"#,
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
        .args(["--preset", "test-preset", "--starship", &stub, &user_config])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    let cache_path = stdout.trim();

    // Merged: preset is the base, user config overrides
    let cached_toml = fs::read_to_string(cache_path).unwrap();
    insta::assert_snapshot!(cached_toml);
}
