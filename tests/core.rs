use cvm::{
    cvm_home_from_env, env_script, init_script, install_prefix_for_home, parse_remote_index_latest,
    parse_remote_index_versions, parse_tool_spec, Tool, ToolSpec, Version,
};
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

#[test]
fn parses_stable_and_rc_versions() {
    assert_eq!(Version::parse("21.1.8").unwrap().to_string(), "21.1.8");
    assert_eq!(
        Version::parse("21.1.0-rc1").unwrap().to_string(),
        "21.1.0-rc1"
    );
    assert!(Version::parse("21").is_err());
    assert!(Version::parse("21.1.x").is_err());
}

#[test]
fn orders_rc_before_stable_release() {
    let rc = Version::parse("21.1.0-rc1").unwrap();
    let stable = Version::parse("21.1.0").unwrap();
    let newer = Version::parse("21.1.8").unwrap();

    assert!(rc < stable);
    assert!(stable < newer);
}

#[test]
fn parses_tool_specs() {
    assert_eq!(
        parse_tool_spec("llvm@21.1.8").unwrap(),
        ToolSpec {
            tool: Tool::Llvm,
            version: Version::parse("21.1.8").unwrap()
        }
    );
    assert_eq!(parse_tool_spec("gcc@15.1.0").unwrap().tool, Tool::Gcc);
    assert!(parse_tool_spec("clang@21.1.8").is_err());
}

#[test]
fn emits_path_only_env_for_llvm_and_gcc() {
    let llvm = env_script(Tool::Llvm, Path::new("/opt/cvm/toolchains/llvm/21.1.8"));
    assert!(llvm.contains("_cvm_strip_toolchain_paths"));
    assert!(llvm.contains("toolchains/llvm/*/bin"));
    assert!(!llvm.contains("toolchains/gcc/*/bin"));
    assert!(llvm.contains("export PATH=\"/opt/cvm/toolchains/llvm/21.1.8/bin:$PATH\""));

    let gcc = env_script(Tool::Gcc, Path::new("/opt/cvm/toolchains/gcc/15.1.0"));
    assert!(gcc.contains("_cvm_strip_toolchain_paths"));
    assert!(gcc.contains("toolchains/gcc/*/bin"));
    assert!(!gcc.contains("toolchains/llvm/*/bin"));
    assert!(gcc.contains("export PATH=\"/opt/cvm/toolchains/gcc/15.1.0/bin:$PATH\""));

    for script in [&llvm, &gcc] {
        for variable in ["CC", "CXX", "LD", "LLVM", "HOSTCC", "HOSTCXX"] {
            assert!(!script.contains(&format!("export {variable}=")));
            assert!(!script.contains(&format!("unset {variable}")));
        }
    }
}

#[test]
fn evaluating_env_script_preserves_user_build_variables_and_other_tool_family() {
    let script = env_script(Tool::Llvm, Path::new("/opt/cvm/toolchains/llvm/21.1.8"));
    let output = Command::new("bash")
        .arg("-c")
        .arg(
            "eval \"$CVM_TEST_SCRIPT\"\n\
             printf '%s\\n' \"$CC|$CXX|$LD|$LLVM|$HOSTCC|$HOSTCXX\"\n\
             printf '%s\\n' \"$PATH\"",
        )
        .env("CVM_TEST_SCRIPT", script)
        .env("CVM_HOME", "/opt/cvm")
        .env(
            "PATH",
            "/opt/cvm/toolchains/llvm/20.1.8/bin:/opt/cvm/toolchains/gcc/15.1.0/bin:/usr/bin",
        )
        .env("CC", "ccache gcc")
        .env("CXX", "ccache g++")
        .env("LD", "gold")
        .env("LLVM", "1")
        .env("HOSTCC", "host-gcc")
        .env("HOSTCXX", "host-g++")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut lines = stdout.lines();
    assert_eq!(
        lines.next(),
        Some("ccache gcc|ccache g++|gold|1|host-gcc|host-g++")
    );
    assert_eq!(
        lines.next(),
        Some("/opt/cvm/toolchains/llvm/21.1.8/bin:/opt/cvm/toolchains/gcc/15.1.0/bin:/usr/bin")
    );
}

#[test]
fn init_script_loads_defaults_dynamically_and_wraps_use() {
    let script = init_script(&[(
        Tool::Llvm,
        Version::parse("21.1.8").unwrap(),
        Path::new("/opt/cvm/toolchains/llvm/21.1.8").to_path_buf(),
    )]);

    assert!(!script.contains("export LLVM=\"/opt/cvm/toolchains/llvm/21.1.8/bin/\""));
    assert!(script.contains("cvm() {"));
    assert!(script.contains("shift"));
    assert!(script.contains("eval \"$(command cvm use \"$@\")\""));
    assert!(script.contains("export PATH=\"$CVM_HOME/bin:$PATH\""));
    assert!(script.contains("command cvm env --defaults"));
    assert!(script.contains("command cvm completion bash"));
    assert!(script.contains("command cvm completion zsh"));
}

#[test]
fn default_home_is_home_dot_cvm_and_prefixes_live_under_toolchains() {
    let home = OsString::from("/home/alice");
    let cvm_home = cvm_home_from_env(None, Some(&home)).unwrap();
    assert_eq!(cvm_home, Path::new("/home/alice/.cvm"));

    let empty = OsString::from("");
    let cvm_home = cvm_home_from_env(Some(&empty), Some(&home)).unwrap();
    assert_eq!(cvm_home, Path::new("/home/alice/.cvm"));

    let prefix = install_prefix_for_home(&cvm_home, Tool::Llvm, &Version::parse("21.1.8").unwrap());
    assert_eq!(prefix, Path::new("/home/alice/.cvm/toolchains/llvm/21.1.8"));
}

#[test]
fn parses_remote_index_versions_dates_and_urls() {
    let index = r#"
{
  "schema_version": 1,
  "generated_at": "2026-06-10T00:00:00Z",
  "cvm": {"latest": "v0.1.0"},
  "compilers": {
    "gcc": [
      {"version": "14.2.0", "date": "2024-08-01", "url": "https://ftp.gnu.org/gnu/gcc/gcc-14.2.0/gcc-14.2.0.tar.xz"},
      {"version": "15.1.0", "date": "2025-04-25", "url": "https://ftp.gnu.org/gnu/gcc/gcc-15.1.0/gcc-15.1.0.tar.xz"}
    ],
    "llvm": [
      {"version": "10.0.1", "date": "2020-08-06", "url": "https://github.com/llvm/llvm-project/releases/download/llvmorg-10.0.1/llvm-project-10.0.1.tar.xz"},
      {"version": "21.1.8", "date": "2026-01-07", "url": "https://github.com/llvm/llvm-project/releases/download/llvmorg-21.1.8/llvm-project-21.1.8.src.tar.xz"}
    ]
  }
}
"#;

    assert_eq!(
        parse_remote_index_latest(index).unwrap().to_string(),
        "0.1.0"
    );

    let gcc = parse_remote_index_versions(index, Tool::Gcc).unwrap();
    assert_eq!(gcc[0].version.to_string(), "15.1.0");
    assert_eq!(gcc[0].date.as_deref(), Some("2025-04-25"));
    assert!(gcc[0].url.ends_with("gcc-15.1.0.tar.xz"));

    let llvm = parse_remote_index_versions(index, Tool::Llvm).unwrap();
    assert_eq!(llvm[0].version.to_string(), "21.1.8");
    assert_eq!(llvm[0].date.as_deref(), Some("2026-01-07"));
    assert!(llvm[0].url.ends_with("llvm-project-21.1.8.src.tar.xz"));
    assert!(llvm[1].url.ends_with("llvm-project-10.0.1.tar.xz"));
}
