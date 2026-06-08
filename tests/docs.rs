use std::fs;

#[test]
fn docs_use_qgrain_repository_and_initial_version() {
    let readme = fs::read_to_string("README.md").unwrap();
    let readme_cn = fs::read_to_string("README_CN.md").unwrap();
    let install = fs::read_to_string("install.sh").unwrap();

    for text in [&readme, &readme_cn, &install] {
        assert!(text.contains("QGrain/cvm"));
        assert!(!text.contains("olduser/cvm"));
    }

    assert!(readme.contains("v0.0.1"));
    assert!(readme_cn.contains("v0.0.1"));
    assert!(readme.contains("$HOME/.cvm"));
    assert!(readme_cn.contains("$HOME/.cvm"));
    assert!(!readme.contains("XDG_DATA_HOME"));
    assert!(!readme_cn.contains("XDG_DATA_HOME"));
    assert!(!readme.contains(".local/bin"));
    assert!(!readme_cn.contains(".local/bin"));
}

#[test]
fn install_script_uses_nvm_style_home_and_profile_loader() {
    let install = fs::read_to_string("install.sh").unwrap();

    assert!(install.contains("cvm_home=\"${CVM_HOME:-${HOME}/.cvm}\""));
    assert!(install.contains("install_dir=\"${cvm_home}/bin\""));
    assert!(install.contains("export CVM_HOME="));
    assert!(install.contains("$CVM_HOME/cvm.sh"));
    assert!(install.contains("PROFILE=/dev/null"));
    assert!(!install.contains(".local/bin"));
}

#[test]
fn install_script_supports_local_checkout_and_source_fallback() {
    let install = fs::read_to_string("install.sh").unwrap();

    assert!(install.contains("cvm_latest_version()"));
    assert!(install.contains("v0.0.1"));
    assert!(install.contains("is_local_checkout()"));
    assert!(install.contains("install_from_local_checkout()"));
    assert!(install.contains("cargo build --release"));
    assert!(install.contains("install_from_binary_asset()"));
    assert!(install.contains("install_from_source_archive()"));
    assert!(install.contains("archive/refs/tags/${version}.tar.gz"));
    assert!(!install.contains("releases/latest/download"));
}
