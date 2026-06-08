use cvm::{
    cvm_home_from_env, env_script, init_script, install_prefix_for_home, parse_tool_spec,
    resolve_active_version, Tool, ToolSpec, Version,
};
use std::ffi::OsString;
use std::path::Path;

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
fn emits_kernel_oriented_env_for_llvm_and_gcc() {
    let llvm = env_script(Tool::Llvm, Path::new("/opt/cvm/toolchains/llvm/21.1.8"));
    assert!(llvm.contains("_cvm_strip_toolchain_paths"));
    assert!(llvm.contains("unset CC CXX LD LLVM HOSTCC HOSTCXX"));
    assert!(llvm.contains("export PATH=\"/opt/cvm/toolchains/llvm/21.1.8/bin:$PATH\""));
    assert!(llvm.contains("export LLVM=\"/opt/cvm/toolchains/llvm/21.1.8/bin/\""));
    assert!(llvm.contains("export LD=\"ld.lld\""));

    let gcc = env_script(Tool::Gcc, Path::new("/opt/cvm/toolchains/gcc/15.1.0"));
    assert!(gcc.contains("_cvm_strip_toolchain_paths"));
    assert!(gcc.contains("unset CC CXX LD LLVM HOSTCC HOSTCXX"));
    assert!(gcc.contains("export CC=\"gcc\""));
    assert!(gcc.contains("export HOSTCC=\"gcc\""));
    assert!(!gcc.contains("export LLVM="));
    assert!(!gcc.contains("export LD="));
}

#[test]
fn local_version_overrides_global_default() {
    assert_eq!(
        resolve_active_version(Some("17.0.6"), Some("21.1.8")),
        Some("17.0.6".to_string())
    );
    assert_eq!(
        resolve_active_version(None, Some("21.1.8")),
        Some("21.1.8".to_string())
    );
    assert_eq!(resolve_active_version(None, None), None);
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
