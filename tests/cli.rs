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
    let bin = home.join("toolchains").join(tool).join(version).join("bin");
    fs::create_dir_all(&bin).unwrap();
    for name in [
        "gcc",
        "g++",
        "gcov",
        "clang",
        "clang++",
        "ld.lld",
        "llvm-ar",
        "llvm-nm",
        "llvm-objcopy",
    ] {
        fs::write(bin.join(name), "").unwrap();
    }
}

fn write_fixture(home: &Path, name: &str, body: &str) -> String {
    let path = home.join(name);
    fs::write(&path, body).unwrap();
    format!("file://{}", path.display())
}

fn write_install_index(home: &Path, tool: &str, version: &str) -> (String, PathBuf) {
    let archive_name = match tool {
        "llvm" => format!("llvm-project-{version}.src.tar.xz"),
        "gcc" => format!("gcc-{version}.tar.xz"),
        _ => panic!("unsupported tool"),
    };
    let archive = home.join(&archive_name);
    fs::write(&archive, "archive").unwrap();
    fs::write(home.join(format!("{archive_name}.sig")), "signature").unwrap();
    let index_url = write_fixture(
        home,
        &format!("remote-index-{tool}-{version}.json"),
        &format!(
            r#"{{
  "schema_version": 1,
  "generated_at": "2026-06-10T00:00:00Z",
  "cvm": {{"latest": "v0.0.7"}},
  "compilers": {{
    "gcc": [{}],
    "llvm": [{}]
  }}
}}"#,
            if tool == "gcc" {
                format!(
                    r#"{{"version": "{version}", "date": "2026-01-01", "url": "file://{}"}}"#,
                    archive.display()
                )
            } else {
                String::new()
            },
            if tool == "llvm" {
                format!(
                    r#"{{"version": "{version}", "date": "2026-01-01", "url": "file://{}"}}"#,
                    archive.display()
                )
            } else {
                String::new()
            }
        ),
    );
    (index_url, archive)
}

fn write_release_key_fixture(home: &Path, tool: &str) -> (String, String) {
    let (env_key, name) = match tool {
        "llvm" => ("CVM_LLVM_RELEASE_KEYS_URL", "release-keys.asc"),
        "gcc" => ("CVM_GCC_RELEASE_KEYS_URL", "gnu-keyring.gpg"),
        _ => panic!("unsupported tool"),
    };
    (
        env_key.to_string(),
        write_fixture(home, name, &format!("{tool} release key")),
    )
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
    let gpg = fake_bin.join("gpg");
    fs::write(
        &gpg,
        r#"#!/bin/bash
set -e
exit 0
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [&bash, &gpg] {
            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }
    format!("{}:{}", fake_bin.display(), env::var("PATH").unwrap())
}

fn fake_bash_path_with_log(home: &Path, log: &Path) -> String {
    let fake_bin = home.join("fake-bin-log");
    fs::create_dir_all(&fake_bin).unwrap();
    let bash = fake_bin.join("bash");
    fs::write(
        &bash,
        format!(
            r#"#!/bin/bash
set -e
printf '%s\n' "$@" > "{}"
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
            log.display()
        ),
    )
    .unwrap();
    let gpg = fake_bin.join("gpg");
    let gpg_log = log.with_file_name("gpg-args.log");
    fs::write(
        &gpg,
        format!(
            r#"#!/bin/bash
set -e
printf '%s\n' "$@" >> "{}"
exit 0
"#,
            gpg_log.display()
        ),
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for path in [&bash, &gpg] {
            let mut permissions = fs::metadata(path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).unwrap();
        }
    }
    format!("{}:{}", fake_bin.display(), env::var("PATH").unwrap())
}

#[test]
fn version_and_help_do_not_expose_removed_kernel_or_source_flags() {
    let home = cvm_home("help");

    let version = run(&home, &["--version"]);
    assert!(version.status.success());
    assert_eq!(String::from_utf8_lossy(&version.stdout).trim(), "cvm 0.0.7");

    let help = run(&home, &["help"]);
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    assert!(help.contains("cvm install <llvm|gcc> <version-or-prefix>"));
    assert!(help.contains("cvm profile template <llvm|gcc>"));
    assert!(help.contains("cvm cache <dir|list|prune>"));
    assert!(help.contains("cvm cache dir"));
    assert!(help.contains("cvm ls-remote [llvm|gcc] [prefix]"));
    assert!(help.contains("cvm which <llvm|gcc> [version-or-prefix]"));
    assert!(help.contains("cvm upgrade [version] [--dry-run]"));
    assert!(help.contains("cvm alias default <llvm|gcc> <version-or-prefix>"));
    assert!(!help.contains("--source"));
    assert!(!help.contains("--minimal"));
    assert!(!help.contains("NAME_OR_PATH"));
    assert!(!help.contains("build-profiles"));
    assert!(!help.contains("verify kernel"));
}

#[test]
fn cache_commands_print_dir_list_and_prune_stale_entries() {
    let home = cvm_home("cache");
    let stale = home.join("cache/sources/llvm/17.0.6");
    let fresh = home.join("cache/sources/gcc/15.2.0");
    fs::create_dir_all(&stale).unwrap();
    fs::create_dir_all(&fresh).unwrap();
    fs::write(stale.join("llvm-project-17.0.6.src.tar.xz"), "old").unwrap();
    fs::write(stale.join(".last-used"), "1").unwrap();
    fs::write(fresh.join("gcc-15.2.0.tar.xz"), "new").unwrap();
    fs::write(fresh.join(".last-used"), "4102444800").unwrap();

    let dir = run(&home, &["cache", "dir"]);
    assert!(dir.status.success());
    assert_eq!(
        String::from_utf8_lossy(&dir.stdout).trim(),
        home.join("cache").display().to_string()
    );

    let list = run(&home, &["cache", "list"]);
    assert!(list.status.success());
    let stdout = String::from_utf8_lossy(&list.stdout);
    assert!(stdout.contains("llvm 17.0.6"));
    assert!(stdout.contains("gcc 15.2.0"));

    let prune = run(&home, &["cache", "prune", "--older-than", "14d"]);
    assert!(prune.status.success());
    assert!(!stale.exists());
    assert!(fresh.exists());
    assert!(String::from_utf8_lossy(&prune.stdout).contains("cache: pruned llvm 17.0.6"));
}

#[test]
fn install_reuses_cached_source_archive_and_passes_archive_to_backend() {
    let home = cvm_home("install-cache");
    let archive = home.join("llvm-project-21.1.8.src.tar.xz");
    fs::write(&archive, "archive").unwrap();
    fs::write(home.join("llvm-project-21.1.8.src.tar.xz.sig"), "signature").unwrap();
    let index_url = write_fixture(
        &home,
        "remote-index.json",
        &format!(
            r#"{{
  "schema_version": 1,
  "generated_at": "2026-06-10T00:00:00Z",
  "cvm": {{"latest": "v0.0.7"}},
  "compilers": {{
    "gcc": [],
    "llvm": [
      {{"version": "21.1.8", "date": "2025-12-01", "url": "file://{}"}}
    ]
  }}
}}"#,
            archive.display()
        ),
    );
    let log = home.join("bash-args.log");
    let path = fake_bash_path_with_log(&home, &log);
    let (key_env, key_url) = write_release_key_fixture(&home, "llvm");

    let first = run_with_env(
        &home,
        &["install", "llvm", "21", "-j1"],
        &[
            ("CVM_REMOTE_INDEX_URL", &index_url),
            (&key_env, &key_url),
            ("PATH", &path),
        ],
    );
    assert!(first.status.success());

    let cached_archive = home
        .join("cache/sources/llvm/21.1.8")
        .join("llvm-project-21.1.8.src.tar.xz");
    let cached_signature = cached_archive.with_file_name("llvm-project-21.1.8.src.tar.xz.sig");
    assert!(cached_archive.exists());
    assert!(cached_signature.exists());
    let args = fs::read_to_string(&log).unwrap();
    assert!(args.contains("--archive\n"));
    assert!(args.contains(&cached_archive.display().to_string()));
    let gpg_args = fs::read_to_string(home.join("gpg-args.log")).unwrap();
    assert!(gpg_args.contains("--verify\n"));
    assert!(gpg_args.contains(&cached_signature.display().to_string()));
    assert!(gpg_args.contains(&cached_archive.display().to_string()));

    fs::remove_file(&archive).unwrap();
    let second = run_with_env(
        &home,
        &["install", "llvm", "21", "-j1"],
        &[
            ("CVM_REMOTE_INDEX_URL", &index_url),
            (&key_env, &key_url),
            ("PATH", &path),
        ],
    );
    assert!(second.status.success());
    assert!(
        String::from_utf8_lossy(&second.stdout).contains("cache: using llvm 21.1.8 source archive")
    );
    assert!(
        String::from_utf8_lossy(&second.stdout).contains("verify: llvm 21.1.8 source signature OK")
    );
}

#[test]
fn install_downloads_imports_and_reuses_release_key_bundle_before_verifying_source() {
    let home = cvm_home("install-key-bundle");
    let (index_url, _archive) = write_install_index(&home, "llvm", "21.1.8");
    let key_url = write_fixture(&home, "release-keys.asc", "release key");
    let log = home.join("bash-args.log");
    let path = fake_bash_path_with_log(&home, &log);

    let first = run_with_env(
        &home,
        &["install", "llvm", "21", "-j1"],
        &[
            ("CVM_REMOTE_INDEX_URL", &index_url),
            ("CVM_LLVM_RELEASE_KEYS_URL", &key_url),
            ("PATH", &path),
        ],
    );
    assert!(first.status.success());
    let stdout = String::from_utf8_lossy(&first.stdout);
    let cached_key = home.join("cache/keys/llvm/release-keys.asc");
    assert!(cached_key.exists());
    assert!(stdout.contains("keys: missing llvm release key bundle"));
    assert!(stdout.contains("keys: downloading llvm release key bundle"));
    assert!(stdout.contains(&cached_key.display().to_string()));
    assert!(stdout.contains("keys: importing llvm release key bundle"));
    assert!(stdout.contains("verify: llvm 21.1.8 source signature OK"));

    let gpg_args = fs::read_to_string(home.join("gpg-args.log")).unwrap();
    let import_pos = gpg_args.find("--import").unwrap();
    let verify_pos = gpg_args.find("--verify").unwrap();
    assert!(import_pos < verify_pos);
    assert!(gpg_args.contains(&cached_key.display().to_string()));

    let second = run_with_env(
        &home,
        &["install", "llvm", "21", "-j1"],
        &[
            ("CVM_REMOTE_INDEX_URL", &index_url),
            ("CVM_LLVM_RELEASE_KEYS_URL", &key_url),
            ("PATH", &path),
        ],
    );
    assert!(second.status.success());
    let stdout = String::from_utf8_lossy(&second.stdout);
    assert!(stdout.contains("keys: using cached llvm release key bundle"));
    assert!(stdout.contains("keys: importing llvm release key bundle"));
}

#[test]
fn install_fails_when_source_signature_is_missing() {
    let home = cvm_home("install-missing-sig");
    let archive = home.join("gcc-15.1.0.tar.xz");
    fs::write(&archive, "archive").unwrap();
    let index_url = write_fixture(
        &home,
        "remote-index.json",
        &format!(
            r#"{{
  "schema_version": 1,
  "generated_at": "2026-06-10T00:00:00Z",
  "cvm": {{"latest": "v0.0.7"}},
  "compilers": {{
    "gcc": [
      {{"version": "15.1.0", "date": "2025-04-25", "url": "file://{}"}}
    ],
    "llvm": []
  }}
}}"#,
            archive.display()
        ),
    );
    let path = fake_bash_path(&home);

    let output = run_with_env(
        &home,
        &["install", "gcc", "15", "-j1"],
        &[("CVM_REMOTE_INDEX_URL", &index_url), ("PATH", &path)],
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to download source signature"));
}

#[test]
fn profile_template_writes_default_file_and_prints_content_and_hint() {
    let home = cvm_home("profile-template");
    let llvm = run(&home, &["profile", "template", "llvm"]);
    assert!(llvm.status.success());
    let llvm_stdout = String::from_utf8_lossy(&llvm.stdout);
    assert!(llvm_stdout.contains("[llvm]"));
    assert!(llvm_stdout.contains("LLVM_ENABLE_ASSERTIONS"));
    assert!(llvm_stdout.contains("profile written to:"));
    assert!(llvm_stdout.contains("profiles/build/llvm/default.toml"));
    assert!(llvm_stdout.contains("future `cvm install llvm ...` commands will use this default profile unless --profile is specified"));

    let default_path = home.join("profiles/build/llvm/default.toml");
    assert!(default_path.exists());
    assert!(fs::read_to_string(&default_path)
        .unwrap()
        .contains("targets = \"X86\""));

    let reject = run(&home, &["profile", "template", "llvm"]);
    assert!(!reject.status.success());
    assert!(String::from_utf8_lossy(&reject.stderr).contains("already exists"));

    let force = run(&home, &["profile", "template", "llvm", "--force"]);
    assert!(force.status.success());
    assert!(fs::read_to_string(default_path).unwrap().contains("[llvm]"));
}

#[test]
fn profile_template_can_write_explicit_path_without_minimal_or_output() {
    let home = cvm_home("profile-template-path");
    let output_path = home.join("profiles/build/gcc/custom.toml");

    let gcc = run(
        &home,
        &["profile", "template", "gcc", output_path.to_str().unwrap()],
    );
    assert!(gcc.status.success());
    let gcc_stdout = String::from_utf8_lossy(&gcc.stdout);
    assert!(gcc_stdout.contains("[gcc]"));
    assert!(gcc_stdout.contains("profile written to:"));
    assert!(gcc_stdout.contains("install with: cvm install gcc <version> --profile"));
    assert!(output_path.exists());

    let minimal = run(&home, &["profile", "template", "gcc", "--minimal"]);
    assert!(!minimal.status.success());
    assert!(String::from_utf8_lossy(&minimal.stderr).contains("unknown profile template option"));

    let output = run(
        &home,
        &["profile", "template", "gcc", "--output", "/tmp/gcc.toml"],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown profile template option"));
}

#[test]
fn profile_list_prints_existing_build_profiles() {
    let home = cvm_home("profile-list");
    let llvm_default = home.join("profiles/build/llvm/default.toml");
    let gcc_custom = home.join("profiles/build/gcc/custom.toml");
    fs::create_dir_all(llvm_default.parent().unwrap()).unwrap();
    fs::create_dir_all(gcc_custom.parent().unwrap()).unwrap();
    fs::write(&llvm_default, "[llvm]\ntargets = \"X86\"\n").unwrap();
    fs::write(&gcc_custom, "[gcc]\nlanguages = \"c,c++\"\n").unwrap();

    let output = run(&home, &["profile", "list"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("build:"));
    assert!(stdout.contains("llvm"));
    assert!(stdout.contains("default"));
    assert!(stdout.contains(&llvm_default.display().to_string()));
    assert!(stdout.contains("gcc"));
    assert!(stdout.contains("custom"));
    assert!(stdout.contains(&gcc_custom.display().to_string()));
}

#[test]
fn profile_list_prints_empty_state_when_no_profiles_exist() {
    let home = cvm_home("profile-list-empty");

    let output = run(&home, &["profile", "list"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&format!(
        "no profiles found under {}",
        home.join("profiles").display()
    )));
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
fn install_dry_run_resolves_remote_version_prefix() {
    let home = cvm_home("install-prefix");
    let index_url = write_fixture(
        &home,
        "remote-index.json",
        r#"{
  "schema_version": 1,
  "generated_at": "2026-06-10T00:00:00Z",
  "cvm": {"latest": "v0.0.3"},
  "compilers": {
    "gcc": [],
    "llvm": [
      {"version": "21.1.0", "date": "2025-09-01", "url": "https://example.com/llvm-21.1.0.tar.xz"},
      {"version": "21.1.8", "date": "2025-12-01", "url": "https://example.com/llvm-21.1.8.tar.xz"},
      {"version": "20.1.8", "date": "2025-08-01", "url": "https://example.com/llvm-20.1.8.tar.xz"}
    ]
  }
}"#,
    );

    let output = run_with_env(
        &home,
        &["install", "llvm", "21", "--dry-run"],
        &[("CVM_REMOTE_INDEX_URL", &index_url)],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("21.1.8"));
    assert!(!stdout.contains("21.1.0 --prefix"));
}

#[test]
fn install_dry_run_uses_default_llvm_profile_when_present() {
    let home = cvm_home("install-llvm-profile");
    let profile = home.join("profiles/build/llvm/default.toml");
    fs::create_dir_all(profile.parent().unwrap()).unwrap();
    fs::write(
        &profile,
        r#"[llvm]
targets = "X86;AArch64"
projects = "clang;lld;compiler-rt"
runtimes = "libcxx;libcxxabi;libunwind"
build_type = "Release"

[llvm.cmake_defines]
LLVM_ENABLE_ASSERTIONS = "ON"
"#,
    )
    .unwrap();

    let output = run(&home, &["install", "llvm", "21.1.8", "--dry-run"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("profile:"));
    assert!(stdout.contains("profiles/build/llvm/default.toml"));
    assert!(stdout.contains("env CVM_LLVM_TARGETS=X86;AArch64"));
    assert!(stdout.contains("env CVM_LLVM_CMAKE_DEFINES=LLVM_ENABLE_ASSERTIONS=ON"));
}

#[test]
fn install_profile_value_is_a_path_not_a_named_profile() {
    let home = cvm_home("install-profile-path-only");
    let old_named_profile = home.join("build-profiles/llvm/full.toml");
    fs::create_dir_all(old_named_profile.parent().unwrap()).unwrap();
    fs::write(
        &old_named_profile,
        r#"[llvm]
targets = "X86"
"#,
    )
    .unwrap();

    let output = run(
        &home,
        &[
            "install",
            "llvm",
            "21.1.8",
            "--profile",
            "full",
            "--dry-run",
        ],
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("failed to read full"));
}

#[test]
fn install_dry_run_uses_explicit_gcc_profile_path() {
    let home = cvm_home("install-gcc-profile");
    let profile = home.join("gcc-custom.toml");
    fs::write(
        &profile,
        r#"[gcc]
languages = "c,c++"
multilib = true
bootstrap = true
configure_args = ["--enable-plugin", "--enable-lto"]
"#,
    )
    .unwrap();

    let output = run(
        &home,
        &[
            "install",
            "gcc",
            "15.1.0",
            "--profile",
            profile.to_str().unwrap(),
            "--dry-run",
        ],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("env CVM_GCC_MULTILIB=true"));
    assert!(stdout.contains("env CVM_GCC_BOOTSTRAP=true"));
    assert!(stdout.contains("env CVM_GCC_CONFIGURE_ARGS=--enable-plugin\\n--enable-lto"));
}

#[test]
fn install_rejects_profile_values_with_newlines() {
    let home = cvm_home("install-profile-newlines");
    let profile = home.join("gcc-newline.toml");
    fs::write(
        &profile,
        r#"[gcc]
languages = "c,c++"
configure_args = ["--enable-plugin\n--enable-lto"]
"#,
    )
    .unwrap();

    let output = run(
        &home,
        &[
            "install",
            "gcc",
            "15.1.0",
            "--profile",
            profile.to_str().unwrap(),
            "--dry-run",
        ],
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must not contain newlines"));
}

#[test]
fn install_rejects_profile_with_targets() {
    let home = cvm_home("install-profile-targets");
    let output = run(
        &home,
        &[
            "install",
            "llvm",
            "21.1.8",
            "--profile",
            "full",
            "--targets",
            "X86;AArch64",
            "--dry-run",
        ],
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--profile cannot be combined"));

    let default_profile = home.join("profiles/build/llvm/default.toml");
    fs::create_dir_all(default_profile.parent().unwrap()).unwrap();
    fs::write(
        &default_profile,
        r#"[llvm]
targets = "X86"
"#,
    )
    .unwrap();
    let output = run(
        &home,
        &[
            "install",
            "llvm",
            "21.1.8",
            "--targets",
            "X86;AArch64",
            "--dry-run",
        ],
    );

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("active build profile"));
}

#[test]
fn install_sets_default_when_first_managed_version_is_installed() {
    let home = cvm_home("install-default");
    let path = fake_bash_path(&home);
    let (index_url, _archive) = write_install_index(&home, "gcc", "15.1.0");
    let (key_env, key_url) = write_release_key_fixture(&home, "gcc");

    let output = run_with_env(
        &home,
        &["install", "gcc", "15", "-j1"],
        &[
            ("PATH", &path),
            ("CVM_REMOTE_INDEX_URL", &index_url),
            (&key_env, &key_url),
        ],
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
    let (index_15, _archive_15) = write_install_index(&home, "gcc", "15.1.0");
    let (index_13, _archive_13) = write_install_index(&home, "gcc", "13.3.0");
    let (key_env, key_url) = write_release_key_fixture(&home, "gcc");
    mark_installed(&home, "gcc", "14.2.0");
    assert!(run(&home, &["alias", "default", "gcc", "14.2.0"])
        .status
        .success());

    let output = run_with_env(
        &home,
        &["install", "gcc", "15", "-j1"],
        &[
            ("PATH", &path),
            ("CVM_REMOTE_INDEX_URL", &index_15),
            (&key_env, &key_url),
        ],
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
        &["install", "gcc", "13", "--prefix", custom.to_str().unwrap()],
        &[
            ("PATH", &path),
            ("CVM_REMOTE_INDEX_URL", &index_13),
            (&key_env, &key_url),
        ],
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

    let alias = run(&home, &["alias", "default", "llvm", "21"]);
    assert!(alias.status.success());
    assert!(String::from_utf8_lossy(&alias.stdout).contains("default llvm -> 21.1.8"));

    let env_output = run(&home, &["env", "llvm"]);
    assert!(env_output.status.success());
    let stdout = String::from_utf8_lossy(&env_output.stdout);
    assert!(stdout.contains("toolchains/llvm/21.1.8/bin"));
}

#[test]
fn env_without_version_requires_global_default() {
    let home = cvm_home("env-no-default");
    mark_installed(&home, "llvm", "21.1.8");

    let output = run(&home, &["env", "llvm"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no default version configured"));
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

    let output = run(&home, &["use", "gcc", "15"]);
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("export CC=\"gcc\""));
    assert!(!home.join("defaults/gcc").exists());
}

#[test]
fn which_prints_managed_compiler_paths_for_prefix() {
    let home = cvm_home("which");
    mark_installed(&home, "llvm", "21.1.0");
    mark_installed(&home, "llvm", "21.1.8");

    let output = run(&home, &["which", "llvm", "21"]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("llvm 21.1.8:"));
    assert!(stdout.contains("clang:"));
    assert!(stdout.contains("toolchains/llvm/21.1.8/bin/clang"));
    assert!(!stdout.contains("21.1.0/bin/clang"));
}

#[test]
fn version_prefix_without_local_match_errors() {
    let home = cvm_home("prefix-no-match");
    mark_installed(&home, "gcc", "14.2.0");

    let output = run(&home, &["use", "gcc", "15"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr)
        .contains("no installed gcc version matches prefix 15"));
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
fn ls_remote_filters_by_version_prefix() {
    let home = cvm_home("ls-remote-prefix");
    let index_url = write_fixture(
        &home,
        "remote-index.json",
        r#"{
  "schema_version": 1,
  "generated_at": "2026-06-10T00:00:00Z",
  "cvm": {"latest": "v0.0.3"},
  "compilers": {
    "gcc": [],
    "llvm": [
      {"version": "21.1.8", "date": "2025-12-01", "url": "https://example.com/llvm-21.1.8.tar.xz"},
      {"version": "20.1.8", "date": "2025-08-01", "url": "https://example.com/llvm-20.1.8.tar.xz"}
    ]
  }
}"#,
    );

    let output = run_with_env(
        &home,
        &["ls-remote", "llvm", "21"],
        &[("CVM_REMOTE_INDEX_URL", &index_url)],
    );

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("21.1.8"));
    assert!(!stdout.contains("20.1.8"));
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
  "cvm": {"latest": "v0.0.8"},
  "compilers": {"gcc": [], "llvm": []}
}"#,
    );

    let output = run_with_env(&home, &["version"], &[("CVM_REMOTE_INDEX_URL", &index_url)]);

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cvm 0.0.7"));
    assert!(stdout.contains("new version available: v0.0.8"));
    assert!(stdout.contains("cvm upgrade"));
    assert!(stdout.contains("diagnostics:"));
    assert!(stdout.contains("CVM_HOME:"));
    assert!(stdout.contains("cvm.sh:"));
    assert!(stdout.contains("default llvm:"));
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
  "cvm": {"latest": "v0.0.7"},
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
    assert!(stdout.contains("upgrade: v0.0.7"));
    assert!(stdout
        .contains("installer: https://raw.githubusercontent.com/QGrain/cvm/v0.0.7/install.sh"));
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

    let output = run(&home, &["uninstall", "llvm", "21"]);
    assert!(output.status.success());
    assert!(!home.join("defaults/llvm").exists());
}
