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

    assert!(readme.contains("v0.0.3"));
    assert!(readme_cn.contains("v0.0.3"));
    assert!(readme.contains("$HOME/.cvm"));
    assert!(readme_cn.contains("$HOME/.cvm"));
    assert!(!readme.contains("XDG_DATA_HOME"));
    assert!(!readme_cn.contains("XDG_DATA_HOME"));
    assert!(!readme.contains(".local/bin"));
    assert!(!readme_cn.contains(".local/bin"));
    let removed_project_defaults_file = [".", "cvm", "rc"].concat();
    assert!(!readme.contains(&removed_project_defaults_file));
    assert!(!readme_cn.contains(&removed_project_defaults_file));
    assert!(!readme.contains("## Repository"));
    assert!(!readme.contains("## Release Assets"));
    assert!(readme.contains("docs/design.md"));
    assert!(readme.contains("docs/release.md"));
    assert!(readme.contains("docs/troubleshooting.md"));
    assert!(readme.contains("docs/contribution.md"));
    assert!(readme_cn.contains("docs/design.md"));
    assert!(readme_cn.contains("docs/release.md"));
    assert!(readme_cn.contains("docs/troubleshooting.md"));
    assert!(readme_cn.contains("docs/contribution.md"));
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
    assert!(install.contains("v0.0.3"));
    assert!(install.contains("is_local_checkout()"));
    assert!(install.contains("install_from_local_checkout()"));
    assert!(install.contains("cargo build --release"));
    assert!(install.contains("install_from_binary_asset()"));
    assert!(install.contains("install_from_source_archive()"));
    assert!(install.contains("archive/refs/tags/${version}.tar.gz"));
    assert!(!install.contains("releases/latest/download"));
}

#[test]
fn http_client_uses_standard_proxy_environment() {
    let manifest = fs::read_to_string("Cargo.toml").unwrap();
    let source = fs::read_to_string("src/lib.rs").unwrap();
    let design = fs::read_to_string("docs/design.md").unwrap();
    let troubleshooting = fs::read_to_string("docs/troubleshooting.md").unwrap();
    let index = fs::read_to_string("manifests/remote-index.json").unwrap();

    assert!(manifest.contains("proxy-from-env"));
    assert!(source.contains("try_proxy_from_env(true)"));
    assert!(source.contains("CVM_REMOTE_INDEX_URL"));
    assert!(design.contains("CVM_REMOTE_INDEX_URL"));
    assert!(troubleshooting.contains("http_proxy"));
    assert!(troubleshooting.contains("https_proxy"));
    assert!(index.contains("\"schema_version\": 1"));
    assert!(index.contains("\"version\""));
    assert!(index.contains("\"date\""));
    assert!(index.contains("\"url\""));
    assert!(!source.contains("api.github.com/repos/llvm/llvm-project/releases"));
    assert!(!source.contains("CVM_GITHUB_TOKEN"));
    assert!(!design.contains("CVM_GITHUB_TOKEN"));
}

#[test]
fn remote_index_synchronization_assets_exist() {
    let script = fs::read_to_string("tools/update_remote_index.py").unwrap();
    let workflow = fs::read_to_string(".github/workflows/synchronize-remote-index.yml").unwrap();
    let index = fs::read_to_string("manifests/remote-index.json").unwrap();

    assert!(script.contains("https://ftp.gnu.org/gnu/gcc/"));
    assert!(script.contains("https://github.com/llvm/llvm-project/releases"));
    assert!(script.contains("LLVM_MIN_VERSION = \"9.0.1\""));
    assert!(script.contains("src.tar.xz"));
    assert!(script.contains("tar.xz"));
    assert!(workflow.contains("37 3 1,15 * *"));
    assert!(workflow.contains("workflow_dispatch"));
    assert!(workflow.contains("Synchronize compiler remote index"));
    assert!(index.contains("llvm-project-21.1.8.src.tar.xz"));
    assert!(index.contains("llvm-project-10.0.1.tar.xz"));
    assert!(!index.contains("\"tag\""));
}

#[test]
fn release_workflow_assets_match_installer_names() {
    let workflow = fs::read_to_string(".github/workflows/release.yml").unwrap();
    let install = fs::read_to_string("install.sh").unwrap();
    let release_docs = fs::read_to_string("docs/release.md").unwrap();
    let contribution = fs::read_to_string("docs/contribution.md").unwrap();

    for target in [
        "x86_64-unknown-linux-musl",
        "aarch64-unknown-linux-musl",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
    ] {
        let asset = format!("cvm-{target}.tar.gz");
        assert!(workflow.contains(&asset));
        assert!(release_docs.contains(&asset));
    }

    assert!(install.contains("cvm-${arch}-${os}.tar.gz"));
    assert!(workflow.contains("gh release create"));
    assert!(workflow.contains("--generate-notes"));
    assert!(workflow.contains("gh release upload"));
    assert!(workflow.contains("--clobber"));
    assert!(workflow.contains("workflow_dispatch"));
    assert!(workflow.contains("Release tag to build, for example v0.0.1"));
    assert!(workflow.contains("RELEASE_TAG"));
    assert!(workflow.contains("ref: ${{ env.RELEASE_TAG }}"));
    assert!(release_docs.contains("backfill"));
    assert!(release_docs.contains("v0.0.1"));
    assert!(contribution.contains("cargo clippy --all-targets -- -D warnings"));
}
