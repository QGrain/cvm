use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn cvm_home(name: &str) -> PathBuf {
    let mut path = env::temp_dir();
    path.push(format!("cvm-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn run(home: &PathBuf, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cvm"))
        .env("CVM_HOME", home)
        .args(args)
        .output()
        .unwrap()
}

fn run_without_cvm_home(home: &PathBuf, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_cvm"))
        .env_remove("CVM_HOME")
        .env_remove("XDG_DATA_HOME")
        .env("HOME", home)
        .args(args)
        .output()
        .unwrap()
}

fn run_with_env(home: &PathBuf, args: &[&str], envs: &[(&str, &str)]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_cvm"));
    command.env("CVM_HOME", home).args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().unwrap()
}

fn mark_installed(home: &Path, tool: &str, version: &str) {
    fs::create_dir_all(home.join("toolchains").join(tool).join(version).join("bin")).unwrap();
}

fn write_fixture(home: &Path, name: &str, body: &str) -> String {
    let path = home.join(name);
    fs::write(&path, body).unwrap();
    format!("file://{}", path.display())
}

fn fake_bash_path(home: &Path) -> String {
    let fake_bin = home.join("fake-bin");
    fs::create_dir_all(&fake_bin).unwrap();
    let bash = fake_bin.join("bash");
    fs::write(
        &bash,
        r#"#!/bin/bash
set -e
prefix=""
prev=""
for arg in "$@"; do
  if [ "$prev" = "--prefix" ]; then
    prefix="$arg"
  fi
  prev="$arg"
done
mkdir -p "$prefix/bin"
touch "$prefix/bin/gcc" "$prefix/bin/g++" "$prefix/bin/clang" "$prefix/bin/clang++"
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(&bash).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&bash, permissions).unwrap();
    }
    format!("{}:{}", fake_bin.display(), env::var("PATH").unwrap())
}

#[test]
fn version_and_help_do_not_expose_removed_kernel_or_source_flags() {
    let home = cvm_home("help");

    let version = run(&home, &["--version"]);
    assert!(version.status.success());
    assert_eq!(String::from_utf8_lossy(&version.stdout).trim(), "cvm 0.0.2");

    let help = run(&home, &["help"]);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("cvm install <llvm|gcc> <version>"));
    assert!(help.contains("cvm ls-remote [llvm|gcc]"));
    assert!(help.contains("cvm upgrade [version] [--dry-run]"));
    assert!(help.contains("cvm alias default <llvm|gcc> <version>"));
    assert!(!help.contains("--source"));
    assert!(!help.contains("verify kernel"));
}

#[test]
fn install_dry_run_uses_embedded_build_backend_without_passthrough_separator() {
    let home = cvm_home("install");
    let output = run(&home, &["install", "llvm", "21.1.8", "-j8", "--dry-run"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("build_llvm-project.sh"));
    assert!(stdout.contains("21.1.8"));
    assert!(stdout.contains("-j8"));
    assert!(stdout.contains("--prefix"));
    assert!(!stdout.contains("--source"));
}

#[test]
fn install_sets_default_when_first_managed_version_is_installed() {
    let home = cvm_home("install-default");
    let path = fake_bash_path(&home);

    let output = run_with_env(
        &home,
        &["install", "gcc", "15.1.0", "-j1"],
        &[("PATH", &path)],
    );

    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(home.join("defaults/gcc"))
            .unwrap()
            .trim(),
        "15.1.0"
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("default gcc -> 15.1.0"));
}

#[test]
fn install_does_not_override_existing_default_or_custom_prefix() {
    let home = cvm_home("install-no-default");
    let path = fake_bash_path(&home);
    mark_installed(&home, "gcc", "14.2.0");
    assert!(run(&home, &["alias", "default", "gcc", "14.2.0"])
        .status
        .success());

    let output = run_with_env(
        &home,
        &["install", "gcc", "15.1.0", "-j1"],
        &[("PATH", &path)],
    );
    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(home.join("defaults/gcc"))
            .unwrap()
            .trim(),
        "14.2.0"
    );

    let custom = home.join("custom-gcc");
    let output = run_with_env(
        &home,
        &[
            "install",
            "gcc",
            "13.3.0",
            "--prefix",
            custom.to_str().unwrap(),
        ],
        &[("PATH", &path)],
    );
    assert!(output.status.success());
    assert_eq!(
        fs::read_to_string(home.join("defaults/gcc"))
            .unwrap()
            .trim(),
        "14.2.0"
    );
}

#[test]
fn alias_default_persists_and_env_uses_it() {
    let home = cvm_home("alias");
    mark_installed(&home, "llvm", "21.1.8");

    let alias = run(&home, &["alias", "default", "llvm", "21.1.8"]);
    assert!(alias.status.success());

    let env_output = run(&home, &["env", "llvm"]);
    assert!(env_output.status.success());
    let stdout = String::from_utf8_lossy(&env_output.stdout);
    assert!(stdout.contains("toolchains/llvm/21.1.8/bin"));
}

#[test]
fn default_home_without_cvm_home_is_home_dot_cvm() {
    let home = cvm_home("default-home-parent");
    let output = run_without_cvm_home(&home, &["install", "llvm", "21.1.8", "--dry-run"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(".cvm/toolchains/llvm/21.1.8"));
    assert!(!stdout.contains(".local/share/cvm"));
}

#[test]
fn env_defaults_prints_all_persisted_defaults() {
    let home = cvm_home("env-defaults");
    mark_installed(&home, "llvm", "21.1.8");
    mark_installed(&home, "gcc", "15.1.0");
    assert!(run(&home, &["alias", "default", "llvm", "21.1.8"])
        .status
        .success());
    assert!(run(&home, &["alias", "default", "gcc", "15.1.0"])
        .status
        .success());

    let output = run(&home, &["env", "--defaults"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("toolchains/llvm/21.1.8/bin"));
    assert!(stdout.contains("toolchains/gcc/15.1.0/bin"));
    assert_eq!(
        stdout
            .matches("unset CC CXX LD LLVM HOSTCC HOSTCXX")
            .count(),
        1
    );
}

#[test]
fn use_prints_temporary_shell_environment_without_setting_default() {
    let home = cvm_home("use");
    mark_installed(&home, "gcc", "15.1.0");

    let output = run(&home, &["use", "gcc", "15.1.0"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("export CC=\"gcc\""));
    assert!(!home.join("defaults/gcc").exists());
}

#[test]
fn ls_remote_uses_fixture_sources_and_prints_compatibility_note() {
    let home = cvm_home("ls-remote");
    let index_url = write_fixture(
        &home,
        "remote-index.json",
        r#"{
  "schema_version": 1,
  "generated_at": "2026-06-10T00:00:00Z",
  "cvm": {"latest": "v0.0.2"},
  "compilers": {
    "gcc": [
      {"version": "15.1.0", "date": "2025-04-25", "url": "https://ftp.gnu.org/gnu/gcc/gcc-15.1.0/gcc-15.1.0.tar.xz"}
    ],
    "llvm": [
      {"version": "21.1.0", "date": "2025-09-01", "url": "https://github.com/llvm/llvm-project/releases/download/llvmorg-21.1.0/llvm-project-21.1.0.src.tar.xz"}
    ]
  }
}"#,
    );

    let output = run_with_env(
        &home,
        &["ls-remote"],
        &[("CVM_REMOTE_INDEX_URL", &index_url)],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gcc:"));
    assert!(stdout.contains("15.1.0"));
    assert!(stdout.contains("2025-04-25"));
    assert!(stdout.contains("llvm:"));
    assert!(stdout.contains("21.1.0"));
    assert!(stdout.contains("compatibility:"));
}

#[test]
fn version_checks_latest_release_without_failing_on_network_success() {
    let home = cvm_home("version-check");
    let index_url = write_fixture(
        &home,
        "remote-index.json",
        r#"{
  "schema_version": 1,
  "generated_at": "2026-06-10T00:00:00Z",
  "cvm": {"latest": "v0.0.3"},
  "compilers": {"gcc": [], "llvm": []}
}"#,
    );

    let output = run_with_env(&home, &["version"], &[("CVM_REMOTE_INDEX_URL", &index_url)]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cvm 0.0.2"));
    assert!(stdout.contains("new version available: v0.0.3"));
    assert!(stdout.contains("cvm upgrade"));
}

#[test]
fn upgrade_dry_run_uses_remote_index_latest_when_version_is_omitted() {
    let home = cvm_home("upgrade-latest");
    let index_url = write_fixture(
        &home,
        "remote-index.json",
        r#"{
  "schema_version": 1,
  "generated_at": "2026-06-10T00:00:00Z",
  "cvm": {"latest": "v0.0.3"},
  "compilers": {"gcc": [], "llvm": []}
}"#,
    );

    let output = run_with_env(
        &home,
        &["upgrade", "--dry-run"],
        &[("CVM_REMOTE_INDEX_URL", &index_url)],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("upgrade: v0.0.3"));
    assert!(stdout
        .contains("installer: https://raw.githubusercontent.com/QGrain/cvm/v0.0.3/install.sh"));
}

#[test]
fn upgrade_dry_run_prints_installer_and_asset() {
    let home = cvm_home("upgrade");
    let output = run(&home, &["upgrade", "v0.0.2", "--dry-run"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout
        .contains("installer: https://raw.githubusercontent.com/QGrain/cvm/v0.0.2/install.sh"));
    assert!(stdout.contains("asset: cvm-"));
    assert!(stdout.contains(".tar.gz"));
}

#[test]
fn use_and_alias_reject_uninstalled_versions() {
    let home = cvm_home("reject-uninstalled");

    let use_output = run(&home, &["use", "llvm", "21.1.8"]);
    assert!(!use_output.status.success());
    assert!(String::from_utf8_lossy(&use_output.stderr).contains("is not installed"));

    let alias_output = run(&home, &["alias", "default", "gcc", "15.1.0"]);
    assert!(!alias_output.status.success());
    assert!(String::from_utf8_lossy(&alias_output.stderr).contains("is not installed"));
}

#[test]
fn uninstall_removes_matching_default_alias() {
    let home = cvm_home("uninstall-default");
    mark_installed(&home, "llvm", "21.1.8");

    assert!(run(&home, &["alias", "default", "llvm", "21.1.8"])
        .status
        .success());
    assert!(home.join("defaults/llvm").exists());

    let output = run(&home, &["uninstall", "llvm", "21.1.8"]);
    assert!(output.status.success());
    assert!(!home.join("defaults/llvm").exists());
}
