use std::fs;

#[test]
fn llvm_script_has_safe_defaults_and_expected_options() {
    let script = fs::read_to_string("scripts/build_llvm-project.sh").unwrap();
    assert!(script.contains("set -Eeuo pipefail"));
    assert!(script.contains("--prefix"));
    assert!(script.contains("LLVM_TARGETS_TO_BUILD"));
    assert!(script.contains("clang;lld;compiler-rt"));
    assert!(script.contains("llvm-project-${VERSION}.install"));
    assert!(script.contains("sudo apt install -y"));
    assert!(script.contains("rm -rf \"$src_dir\" \"$archive\""));
    assert!(script.contains("Examples:"));
    assert!(!script.contains("--install-deps"));
    assert!(!script.contains("--clean"));
    assert!(!script.contains("--build-type"));
    assert!(!script.contains("--projects"));
    assert!(!script.contains("--runtimes"));
    assert!(!script.contains("--no-runtimes"));
}

#[test]
fn gcc_script_is_kernel_oriented() {
    let script = fs::read_to_string("scripts/build_gcc.sh").unwrap();
    assert!(script.contains("set -Eeuo pipefail"));
    assert!(script.contains("--enable-languages=c,c++"));
    assert!(script.contains("contrib/download_prerequisites"));
    assert!(script.contains("--disable-multilib"));
    assert!(script.contains("gcc-${VERSION}.install"));
    assert!(script.contains("sudo apt install -y"));
    assert!(script.contains("rm -rf \"$src_dir\" \"$archive\""));
    assert!(script.contains("Examples:"));
    assert!(!script.contains("--install-deps"));
    assert!(!script.contains("--clean"));
    assert!(!script.contains("--mirror"));
    assert!(!script.contains("--build-dir"));
}
