# cvm

[English](README.md)

`cvm` 是一个面向 LLVM 和 GCC 的用户级编译器版本管理工具。

它从源码安装编译器，在当前 shell 中切换编译器版本，记录持久默认版本，并能卸载已安装工具链，不会替换系统编译器。

## 版本

当前版本：`v0.0.2`

## 安装

从本地 checkout 安装：

```sh
git clone https://github.com/QGrain/cvm.git
cd cvm
./install.sh
```

从指定 release tag 安装：

```sh
curl -fsSL https://raw.githubusercontent.com/QGrain/cvm/v0.0.2/install.sh | bash
```

安装器会把 `cvm` 放到 `$HOME/.cvm/bin`，生成 `$HOME/.cvm/cvm.sh`，并向检测到的 shell profile 追加加载片段。设置 `PROFILE=/dev/null` 可以跳过 profile 修改。

如果 `install.sh` 是在本地 checkout 内运行，它会直接对当前 checkout 执行 `cargo build --release`，不会下载 GitHub release asset。如果它是通过下载脚本运行，则会先尝试下载指定 tag 对应的二进制 release asset；如果该 asset 不存在，则回退到 GitHub source archive 并在本地用 Cargo 构建。

源码构建需要 Rust/Cargo 1.85 或更新版本。

使用 `main` 分支安装脚本时，可以显式指定安装 tag：

```sh
curl -fsSL https://raw.githubusercontent.com/QGrain/cvm/main/install.sh | bash -s -- --version v0.0.2
```

## 使用

安装编译器：

```sh
cvm install llvm 21.1.8 -j8
cvm install gcc 15.1.0 -j8
```

当某个编译器类别第一次安装受 cvm 管理的版本时，cvm 会自动把它设置为持久默认版本。已有 default 或使用自定义 `--prefix` 安装时不会被改动。

查看远端可下载版本：

```sh
cvm ls-remote
cvm ls-remote llvm
cvm ls-remote gcc
```

在当前 shell 中使用某个版本：

```sh
cvm use llvm 21.1.8
```

脚本或没有加载 `$CVM_HOME/cvm.sh` 的 shell 中，可以显式执行输出的环境变量：

```sh
eval "$(cvm use llvm 21.1.8)"
```

设置持久默认版本：

```sh
cvm alias default llvm 21.1.8
cvm alias default gcc 15.1.0
```

查看版本：

```sh
cvm ls
cvm current
cvm version
```

检查并升级 cvm 自身：

```sh
cvm version
cvm upgrade --dry-run
cvm upgrade
```

卸载工具链：

```sh
cvm uninstall llvm 17.0.6
```

## 命令

```text
cvm install <llvm|gcc> <version> [-jN|--jobs N]
cvm ls-remote [llvm|gcc]
cvm ls [llvm|gcc]
cvm use <llvm|gcc> [version]
cvm alias default <llvm|gcc> <version>
cvm current [llvm|gcc]
cvm env <llvm|gcc> [version]
cvm uninstall <llvm|gcc> <version>
cvm upgrade [version] [--dry-run]
cvm init
cvm version
```

## Shell 行为

安装器会写入 profile 片段来 source `$HOME/.cvm/cvm.sh`。之后在交互式
shell 中可以像 nvm 一样直接运行 `cvm use ...`。

`cvm use` 和 `cvm alias default` 要求目标版本已经安装。这样可以避免把 `PATH` 指向不存在的工具链，导致实际回退到系统 clang/gcc。

一次性 shell 中仍可使用：

```sh
eval "$(cvm use llvm 21.1.8)"
```

`cvm alias default` 会把默认版本写入 `$CVM_HOME/defaults`。新 shell 通过
source `$CVM_HOME/cvm.sh` 应用默认版本。

切换版本时，cvm 会先清理它自己管理的编译器变量（`CC`、`CXX`、`LD`、`LLVM`、`HOSTCC`、`HOSTCXX`），再导出所选工具链。它不会清理 `CROSS_COMPILE` 这类用户自己管理的无关变量。

## 项目默认版本

可以在项目目录创建 `.cvmrc`：

```text
llvm 21.1.8
gcc 15.1.0
```

当 `cvm use` 或 `cvm env` 没有显式指定版本时，`.cvmrc` 优先于全局默认版本。

## 存储目录

`cvm` 默认把工具链安装到：

```text
$CVM_HOME/toolchains/llvm/<version>
$CVM_HOME/toolchains/gcc/<version>
```

如果没有设置 `CVM_HOME`，则使用 `$HOME/.cvm`。

默认布局：

```text
$HOME/.cvm/bin/cvm
$HOME/.cvm/cvm.sh
$HOME/.cvm/toolchains/llvm/<version>
$HOME/.cvm/toolchains/gcc/<version>
$HOME/.cvm/defaults/{llvm,gcc}
$HOME/.cvm/scripts/
```

## 构建后端

源码构建脚本位于 `scripts/`，并在编译时嵌入 Rust 二进制：

- `scripts/build_llvm-project.sh`
- `scripts/build_gcc.sh`

`cvm install` 会把对应后端写入 `$CVM_HOME/scripts`，再以版本化 prefix 调用。

`cvm ls-remote` 会读取 cvm 仓库中的 `manifests/remote-index.json`。该 manifest 由项目维护，并从上游 GCC 和 LLVM release 页面同步。

`cvm version`、`cvm upgrade`、`cvm ls-remote` 这类网络命令会遵循标准代理环境变量，包括 `HTTP_PROXY`、`HTTPS_PROXY`、`ALL_PROXY` 及其小写形式。

在受限网络中使用 cvm 时，可以设置 `CVM_REMOTE_INDEX_URL` 指向内网镜像的 remote index。

仓库包含一个定时 GitHub Action，会运行 `tools/update_remote_index.py`，并在上游编译器元数据发生变化时创建标题为 `Synchronize compiler remote index` 的 PR。

在 Debian/Ubuntu 系统中，后端脚本会先执行 `sudo apt update` 和 `sudo apt install` 安装构建依赖，然后再编译源码。需要 sudo 凭据时，终端会正常提示输入密码。非交互式环境中，请提前安装依赖或配置 passwordless sudo。

## 开发

```sh
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
bash -n scripts/build_llvm-project.sh
bash -n scripts/build_gcc.sh
python3 tools/update_remote_index.py --output manifests/remote-index.json
```

## 仓库

https://github.com/QGrain/cvm

## Release Assets

`install.sh` 会优先尝试下载以下命名格式的二进制 release asset：

```text
cvm-x86_64-unknown-linux-gnu.tar.gz
cvm-aarch64-unknown-linux-gnu.tar.gz
cvm-x86_64-apple-darwin.tar.gz
cvm-aarch64-apple-darwin.tar.gz
```

每个压缩包的根目录下需要包含一个可执行文件 `cvm`。二进制 asset 是可选的：如果对应 asset 不存在，安装器会下载该 tag 的 GitHub source archive，并在本地用 Cargo 构建 cvm。

## 卸载

先从 shell profile 中移除 source `$CVM_HOME/cvm.sh` 的片段，然后删除：

```sh
rm -rf "$HOME/.cvm"
```

## License

Apache-2.0. 见 [LICENSE](LICENSE)。
