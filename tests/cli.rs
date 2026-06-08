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

fn mark_installed(home: &Path, tool: &str, version: &str) {
    fs::create_dir_all(home.join("toolchains").join(tool).join(version).join("bin")).unwrap();
}

#[test]
fn version_and_help_do_not_expose_removed_kernel_or_source_flags() {
    let home = cvm_home("help");

    let version = run(&home, &["--version"]);
    assert!(version.status.success());
    assert_eq!(
        String::from_utf8_lossy(&version.stdout).trim(),
        concat!("cvm ", env!("CARGO_PKG_VERSION"))
    );

    let help = run(&home, &["help"]);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("cvm install <llvm|gcc> <version>"));
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
