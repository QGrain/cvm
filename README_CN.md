<img src="assets/logos/cvm-logo-color.svg" alt="Compiler Version Manager" width="160">
<br></br>

<h1>Compiler Version Manager</h1>

<img src="https://img.shields.io/badge/version-v0.0.4-orange.svg" alt="Release Version">
<img src="https://img.shields.io/badge/license-Apache--2.0-green" alt="License">
<img src="https://img.shields.io/github/actions/workflow/status/QGrain/cvm/release.yml" alt="Release Workflow Status">
<img src="https://img.shields.io/github/downloads/QGrain/cvm/total" alt="Total Downloads">

[[English]](README.md)

`cvm` 是一个面向 LLVM 和 GCC 的用户级编译器版本管理工具。

它从源码安装编译器工具链，在当前 shell 中切换编译器版本，记录持久默认版本，并且不会替换系统编译器。

## 安装

安装指定 release：

```sh
curl -fsSL https://raw.githubusercontent.com/QGrain/cvm/v0.0.4/install.sh | bash
```

从本地 checkout 安装：

```sh
git clone https://github.com/QGrain/cvm.git
cd cvm
./install.sh
```

安装器会把 `cvm` 写入 `$HOME/.cvm/bin/cvm`，生成 `$HOME/.cvm/cvm.sh`，并向检测到的 shell profile 追加加载片段。设置 `PROFILE=/dev/null` 可以跳过 profile 修改。

重复运行安装器只会替换 cvm binary 并重新生成 `cvm.sh`；`$HOME/.cvm` 下已经安装的工具链和默认版本配置会被保留。

## 快速开始

```sh
cvm install llvm 21 -j8
cvm install gcc 15 -j8

cvm ls-remote llvm 21
cvm ls

cvm use llvm 21
cvm which llvm
cvm alias default llvm 21.1.8

cvm version
cvm upgrade --dry-run

cvm profile template llvm
cvm install llvm 21
```

当某个编译器类别第一次安装受 cvm 管理的版本时，cvm 会自动把它设置为持久默认版本。

## 命令

```text
cvm install <llvm|gcc> <version-or-prefix> [-jN|--jobs N] [--profile PATH] [--targets LIST]
cvm profile template <llvm|gcc> [PATH] [--force]
cvm profile list
cvm ls-remote [llvm|gcc] [prefix]
cvm ls [llvm|gcc]
cvm use <llvm|gcc> [version-or-prefix]
cvm alias default <llvm|gcc> <version-or-prefix>
cvm current [llvm|gcc]
cvm env <llvm|gcc> [version-or-prefix]
cvm which <llvm|gcc> [version-or-prefix]
cvm uninstall <llvm|gcc> <version-or-prefix>
cvm upgrade [version] [--dry-run]
cvm init
cvm version
```

交互式 shell source `$CVM_HOME/cvm.sh` 后，`cvm use ...` 会像 `nvm` 一样直接更新当前 shell。脚本或一次性 shell 中可以使用：

```sh
eval "$(cvm use llvm 21)"
```

## 文档

- [设计说明](docs/design.md)
- [构建配置](docs/build-profiles.md)
- [发布流程](docs/release.md)
- [故障排查](docs/troubleshooting.md)
- [贡献指南](docs/contribution.md)

## 卸载

先从 shell profile 中移除 source `$CVM_HOME/cvm.sh` 的片段，然后删除：

```sh
rm -rf "$HOME/.cvm"
```

## 参与贡献

欢迎参与贡献。提交 PR 前请先阅读 [贡献指南](docs/contribution.md)。

## License

Apache-2.0. 见 [LICENSE](LICENSE)。
