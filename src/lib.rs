use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

pub const CVM_VERSION: &str = env!("CARGO_PKG_VERSION");

const COMPLETION_COMMANDS: &str =
    "install cache profile ls-remote ls list use alias current env which uninstall deactivate upgrade init version help";
const LLVM_BUILD_SCRIPT: &str = include_str!("../scripts/build_llvm-project.sh");
const GCC_BUILD_SCRIPT: &str = include_str!("../scripts/build_gcc.sh");
const DEFAULT_REMOTE_INDEX: &str = include_str!("../manifests/remote-index.json");
const CVM_REPO: &str = "QGrain/cvm";
const DEFAULT_REMOTE_INDEX_URL: &str =
    "https://raw.githubusercontent.com/QGrain/cvm/main/manifests/remote-index.json";
const DEFAULT_CACHE_TTL_SECS: u64 = 14 * 24 * 60 * 60;
const LLVM_RELEASE_KEYS_URL: &str = "https://releases.llvm.org/release-keys.asc";
const GCC_RELEASE_KEYS_URL: &str = "https://ftp.gnu.org/gnu/gnu-keyring.gpg";

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Version {
    major: u32,
    minor: u32,
    patch: u32,
    rc: Option<u32>,
}

impl Version {
    pub fn parse(input: &str) -> Result<Self, String> {
        let (core, rc) = if let Some((core, suffix)) = input.split_once("-rc") {
            if suffix.is_empty() || !suffix.chars().all(|c| c.is_ascii_digit()) {
                return Err(format!("invalid rc version: {input}"));
            }
            (core, Some(parse_u32(suffix, input)?))
        } else if input.contains('-') {
            return Err(format!("unsupported version suffix: {input}"));
        } else {
            (input, None)
        };

        let parts: Vec<&str> = core.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("version must be X.Y.Z or X.Y.Z-rcN: {input}"));
        }

        Ok(Self {
            major: parse_u32(parts[0], input)?,
            minor: parse_u32(parts[1], input)?,
            patch: parse_u32(parts[2], input)?,
            rc,
        })
    }

    fn matches_prefix(&self, prefix: &VersionPrefix) -> bool {
        self.major == prefix.major
            && prefix.minor.is_none_or(|minor| self.minor == minor)
            && prefix.patch.is_none_or(|patch| self.patch == patch)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(rc) = self.rc {
            write!(f, "-rc{rc}")?;
        }
        Ok(())
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (
            self.major,
            self.minor,
            self.patch,
            self.rc_rank(),
            self.rc.unwrap_or(0),
        )
            .cmp(&(
                other.major,
                other.minor,
                other.patch,
                other.rc_rank(),
                other.rc.unwrap_or(0),
            ))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl FromStr for Version {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Version {
    fn rc_rank(&self) -> u8 {
        if self.rc.is_some() {
            0
        } else {
            1
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Tool {
    Llvm,
    Gcc,
}

impl Tool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Llvm => "llvm",
            Self::Gcc => "gcc",
        }
    }

    pub fn all() -> [Self; 2] {
        [Self::Llvm, Self::Gcc]
    }
}

impl fmt::Display for Tool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Tool {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "llvm" => Ok(Self::Llvm),
            "gcc" => Ok(Self::Gcc),
            _ => Err(format!("unsupported compiler family: {s}")),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ToolSpec {
    pub tool: Tool,
    pub version: Version,
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct VersionPrefix {
    major: u32,
    minor: Option<u32>,
    patch: Option<u32>,
}

impl VersionPrefix {
    fn parse(input: &str) -> Result<Self, String> {
        if input.contains('-') {
            return Err(format!(
                "version prefix must be numeric dot components: {input}"
            ));
        }
        let parts: Vec<&str> = input.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(format!("version prefix must be X, X.Y, or X.Y.Z: {input}"));
        }
        Ok(Self {
            major: parse_u32(parts[0], input)?,
            minor: parts
                .get(1)
                .map(|value| parse_u32(value, input))
                .transpose()?,
            patch: parts
                .get(2)
                .map(|value| parse_u32(value, input))
                .transpose()?,
        })
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RemoteVersion {
    pub version: Version,
    pub date: Option<String>,
    pub url: String,
}

struct InstallTarget {
    version: Version,
    source_url: String,
}

struct SourcePackage {
    archive: PathBuf,
    signature: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteIndex {
    schema_version: u32,
    #[serde(rename = "generated_at")]
    _generated_at: String,
    cvm: CvmRemoteIndex,
    compilers: CompilerRemoteIndex,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CvmRemoteIndex {
    latest: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompilerRemoteIndex {
    llvm: Vec<RemoteIndexEntry>,
    gcc: Vec<RemoteIndexEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RemoteIndexEntry {
    version: String,
    date: String,
    url: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BuildProfile {
    llvm: Option<LlvmBuildProfile>,
    gcc: Option<GccBuildProfile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LlvmBuildProfile {
    targets: Option<String>,
    projects: Option<String>,
    runtimes: Option<String>,
    build_type: Option<String>,
    cmake_defines: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GccBuildProfile {
    languages: Option<String>,
    multilib: Option<bool>,
    bootstrap: Option<bool>,
    configure_args: Option<Vec<String>>,
}

pub fn parse_tool_spec(input: &str) -> Result<ToolSpec, String> {
    let (tool, version) = input
        .split_once('@')
        .ok_or_else(|| format!("tool spec must look like llvm@21.1.8 or gcc@15.1.0: {input}"))?;
    Ok(ToolSpec {
        tool: Tool::from_str(tool)?,
        version: Version::parse(version)?,
    })
}

fn parse_tool_spec_request(input: &str) -> Result<ToolSpec, String> {
    let (tool, version) = input
        .split_once('@')
        .ok_or_else(|| format!("tool spec must look like llvm@21.1.8 or gcc@15.1.0: {input}"))?;
    let tool = Tool::from_str(tool)?;
    Ok(ToolSpec {
        tool,
        version: resolve_requested_version(tool, Some(version))?,
    })
}

pub fn parse_remote_index_versions(input: &str, tool: Tool) -> Result<Vec<RemoteVersion>, String> {
    let index = parse_remote_index(input)?;
    Ok(remote_versions_from_index(&index, tool))
}

pub fn parse_remote_index_latest(input: &str) -> Result<Version, String> {
    let index = parse_remote_index(input)?;
    parse_cvm_tag(&index.cvm.latest)
}

pub fn env_script(tool: Tool, prefix: &Path) -> String {
    let bin = prefix.join("bin");
    let bin = shell_escape_path(&bin);
    format!(
        "{}export PATH=\"{bin}:$PATH\"\n",
        strip_toolchain_paths_script(Some(tool))
    )
}

fn system_env_script(tool: Option<Tool>) -> String {
    strip_toolchain_paths_script(tool)
}

pub fn init_script(_defaults: &[(Tool, Version, PathBuf)]) -> String {
    let mut script = String::new();
    script.push_str("# cvm shell integration\n");
    script.push_str(
        r#"
export CVM_HOME="${CVM_HOME:-$HOME/.cvm}"
case ":$PATH:" in
  *":$CVM_HOME/bin:"*) ;;
  *) export PATH="$CVM_HOME/bin:$PATH" ;;
esac

if command -v cvm >/dev/null 2>&1; then
  eval "$(command cvm env --defaults)"
fi

cvm() {
  if [ "$#" -ge 1 ] && [ "$1" = "use" ]; then
    shift
    eval "$(command cvm use "$@")"
  else
    command cvm "$@"
  fi
}

if [ -n "${BASH_VERSION:-}" ]; then
  eval "$(command cvm completion bash)"
elif [ -n "${ZSH_VERSION:-}" ]; then
  eval "$(command cvm completion zsh)"
fi
"#,
    );
    script
}

pub fn cvm_home_from_env(
    cvm_home: Option<&OsString>,
    home: Option<&OsString>,
) -> Result<PathBuf, String> {
    if let Some(home) = cvm_home.filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home));
    }
    if let Some(home) = home.filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(home).join(".cvm"));
    }
    Err("CVM_HOME or HOME must be set".into())
}

pub fn install_prefix_for_home(home: &Path, tool: Tool, version: &Version) -> PathBuf {
    home.join("toolchains")
        .join(tool.as_str())
        .join(version.to_string())
}

pub fn run_cli<I, S>(args: I) -> i32
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    match run_cli_result(args.into_iter().map(Into::into).collect()) {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("cvm: {err}");
            1
        }
    }
}

pub fn run_cli_result(args: Vec<String>) -> Result<(), String> {
    let mut args = args.into_iter();
    let _program = args.next();
    let Some(command) = args.next() else {
        print_help();
        return Ok(());
    };
    let rest: Vec<String> = args.collect();

    match command.as_str() {
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        "--version" | "-V" => {
            println!("cvm {CVM_VERSION}");
            Ok(())
        }
        "version" => cmd_version(&rest),
        "install" => cmd_install(&rest),
        "cache" => cmd_cache(&rest),
        "profile" => cmd_profile(&rest),
        "ls-remote" => cmd_ls_remote(&rest),
        "ls" | "list" => cmd_list(&rest),
        "use" => cmd_use(&rest),
        "env" => cmd_env(&rest),
        "alias" => cmd_alias(&rest),
        "current" => cmd_current(&rest),
        "which" => cmd_which(&rest),
        "uninstall" => cmd_uninstall(&rest),
        "deactivate" => cmd_deactivate(&rest),
        "upgrade" => cmd_upgrade(&rest),
        "init" => cmd_init(&rest),
        "completion" => cmd_completion(&rest),
        other => Err(format!("unknown command: {other}")),
    }
}

fn cmd_version(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("usage: cvm version".into());
    }
    println!("cvm {CVM_VERSION}");
    match latest_cvm_release() {
        Ok(latest) => {
            let current = Version::parse(CVM_VERSION)?;
            if latest > current {
                println!("new version available: v{latest}");
                println!("run: cvm upgrade");
            } else {
                println!("cvm is up to date");
            }
        }
        Err(err) => println!("warning: failed to check remote index: {err}"),
    }
    print_version_diagnostics();
    Ok(())
}

fn cmd_install(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: cvm install <llvm|gcc> <version-or-prefix> [-jN|--jobs N] [--profile PATH] [--targets LIST] [--prefix DIR] [--dry-run]".into());
    }

    let tool = Tool::from_str(&args[0])?;
    let target = resolve_remote_or_exact_install_target(tool, &args[1])?;
    let version = target.version;
    let mut options = InstallOptions::default();
    let mut explicit_targets = false;
    let mut idx = 2;
    while idx < args.len() {
        match args[idx].as_str() {
            "--source" => {
                return Err("source builds are the default; remove --source".into());
            }
            "--" => {
                return Err("cvm no longer accepts passthrough arguments after --; use cvm install options directly".into());
            }
            "-j" | "--jobs" => {
                idx += 1;
                options.jobs = Some(args.get(idx).ok_or("missing jobs value")?.clone());
            }
            value if value.starts_with("-j") && value.len() > 2 => {
                options.jobs = Some(value[2..].to_string());
            }
            "--targets" => {
                idx += 1;
                let targets = args.get(idx).ok_or("missing targets value")?.clone();
                if tool != Tool::Llvm {
                    return Err("--targets is only supported for llvm installs".into());
                }
                options.targets = Some(targets);
                explicit_targets = true;
            }
            "--profile" => {
                idx += 1;
                options.profile = Some(PathBuf::from(args.get(idx).ok_or("missing profile path")?));
            }
            "--prefix" => {
                idx += 1;
                options.prefix = Some(PathBuf::from(args.get(idx).ok_or("missing prefix value")?));
            }
            "--force-configure" => options.force_configure = true,
            "--dry-run" => options.dry_run = true,
            other => return Err(format!("unknown install option: {other}")),
        }
        idx += 1;
    }
    if explicit_targets && options.profile.is_some() {
        return Err("--profile cannot be combined with --targets".into());
    }
    let active_profile = match &options.profile {
        Some(profile) => Some(profile.clone()),
        None => {
            let default = default_build_profile_path(tool)?;
            default.exists().then_some(default)
        }
    };
    if explicit_targets && active_profile.is_some() {
        return Err("--targets cannot be combined with an active build profile".into());
    }

    let using_custom_prefix = options.prefix.is_some();
    let prefix = options
        .prefix
        .clone()
        .unwrap_or(install_prefix(tool, &version)?);
    fs::create_dir_all(prefix.parent().ok_or("invalid install prefix")?)
        .map_err(|e| format!("failed to create install root: {e}"))?;
    let script = ensure_build_script(tool)?;

    let mut command_args = vec![
        script.display().to_string(),
        version.to_string(),
        "--prefix".to_string(),
        prefix.display().to_string(),
    ];
    if let Some(jobs) = options.jobs {
        command_args.push(format!("-j{jobs}"));
    }
    if let Some(targets) = options.targets {
        command_args.push("--targets".to_string());
        command_args.push(targets);
    }
    if options.force_configure {
        command_args.push("--force-configure".to_string());
    }
    let profile_env = match &active_profile {
        Some(profile) => build_profile_env(tool, profile)?,
        None => Vec::new(),
    };

    if options.dry_run {
        if let Some(profile) = &active_profile {
            println!("profile: {}", profile.display());
        }
        for (key, value) in &profile_env {
            println!("env {key}={}", value.replace('\n', "\\n"));
        }
        println!("bash {}", command_args.join(" "));
        return Ok(());
    }

    prune_cache_older_than(DEFAULT_CACHE_TTL_SECS, true)?;
    let source = ensure_cached_source_package(tool, &version, &target.source_url)?;
    ensure_source_release_keys(tool)?;
    verify_source_signature(tool, &version, &source)?;
    command_args.push("--archive".to_string());
    command_args.push(source.archive.display().to_string());

    let mut command = Command::new("bash");
    command.args(command_args);
    command.envs(profile_env);
    run_command(command)?;
    maybe_alias_default_after_install(tool, &version, using_custom_prefix)
}

fn cmd_cache(args: &[String]) -> Result<(), String> {
    let usage = "usage: cvm cache <dir|list|prune>";
    match args.first().map(String::as_str) {
        Some("dir") => {
            if args.len() != 1 {
                return Err("usage: cvm cache dir".into());
            }
            println!("{}", cache_root()?.display());
            Ok(())
        }
        Some("list") => {
            if args.len() != 1 {
                return Err("usage: cvm cache list".into());
            }
            list_cache()
        }
        Some("prune") => cmd_cache_prune(&args[1..]),
        Some("-h") | Some("--help") => {
            println!("{usage}");
            println!("       cvm cache dir");
            println!("       cvm cache list");
            println!("       cvm cache prune [--older-than 14d]");
            Ok(())
        }
        _ => Err(usage.into()),
    }
}

fn cmd_cache_prune(args: &[String]) -> Result<(), String> {
    let mut older_than = DEFAULT_CACHE_TTL_SECS;
    let mut idx = 0;
    while idx < args.len() {
        match args[idx].as_str() {
            "--older-than" => {
                idx += 1;
                older_than = parse_duration_arg(args.get(idx).ok_or("missing duration value")?)?;
            }
            "-h" | "--help" => {
                println!("usage: cvm cache prune [--older-than 14d]");
                return Ok(());
            }
            other => return Err(format!("unknown cache prune option: {other}")),
        }
        idx += 1;
    }
    let pruned = prune_cache_older_than(older_than, true)?;
    if pruned == 0 {
        println!("cache: nothing to prune");
    }
    Ok(())
}

fn cmd_profile(args: &[String]) -> Result<(), String> {
    let usage = "usage: cvm profile <template|list> ...";
    match args.first().map(String::as_str) {
        Some("template") => cmd_profile_template(args),
        Some("list") => cmd_profile_list(&args[1..]),
        Some("-h") | Some("--help") => {
            println!("{usage}");
            println!("       cvm profile template <llvm|gcc> [PATH] [--force]");
            println!("       cvm profile list");
            Ok(())
        }
        _ => Err(usage.into()),
    }
}

fn cmd_profile_template(args: &[String]) -> Result<(), String> {
    if args.get(1).map(String::as_str) == Some("-h")
        || args.get(1).map(String::as_str) == Some("--help")
    {
        println!("usage: cvm profile template <llvm|gcc> [PATH] [--force]");
        return Ok(());
    }
    if args.len() < 2 {
        return Err("usage: cvm profile template <llvm|gcc> [PATH] [--force]".into());
    }
    let tool = Tool::from_str(&args[1])?;
    let mut output = None::<PathBuf>;
    let mut force = false;
    let mut idx = 2;
    while idx < args.len() {
        match args[idx].as_str() {
            "--force" => force = true,
            "-h" | "--help" => {
                println!("usage: cvm profile template <llvm|gcc> [PATH] [--force]");
                return Ok(());
            }
            other if other.starts_with('-') => {
                return Err(format!("unknown profile template option: {other}"));
            }
            path => {
                if output.is_some() {
                    return Err("usage: cvm profile template <llvm|gcc> [PATH] [--force]".into());
                }
                output = Some(PathBuf::from(path));
            }
        }
        idx += 1;
    }

    let template = profile_template(tool);
    parse_build_profile(template)?;
    let path = output.unwrap_or(default_build_profile_path(tool)?);
    if path.exists() && !force {
        return Err(format!(
            "{} already exists; use --force to overwrite",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    fs::write(&path, template).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    print!("{template}");
    println!("\nprofile written to: {}", path.display());
    if path == default_build_profile_path(tool)? {
        println!(
            "future `cvm install {tool} ...` commands will use this default profile unless --profile is specified"
        );
    } else {
        println!(
            "install with: cvm install {tool} <version> --profile {}",
            path.display()
        );
    }
    Ok(())
}

fn cmd_profile_list(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("usage: cvm profile list".into());
    }
    let profiles = cvm_home()?.join("profiles");
    if !profiles.exists() {
        println!("no profiles found under {}", profiles.display());
        return Ok(());
    }

    let mut entries = Vec::<(Tool, String, PathBuf)>::new();
    for tool in Tool::all() {
        let dir = profiles.join("build").join(tool.as_str());
        let read_dir = match fs::read_dir(&dir) {
            Ok(read_dir) => read_dir,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(format!("failed to read {}: {err}", dir.display())),
        };
        let mut tool_entries = Vec::new();
        for entry in read_dir {
            let entry = entry.map_err(|err| format!("failed to read {}: {err}", dir.display()))?;
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            tool_entries.push((name.to_string(), path));
        }
        tool_entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries.extend(
            tool_entries
                .into_iter()
                .map(|(name, path)| (tool, name, path)),
        );
    }

    if entries.is_empty() {
        println!("no profiles found under {}", profiles.display());
        return Ok(());
    }

    println!("build:");
    for (tool, name, path) in entries {
        println!("  {:<4} {:<8} {}", tool, name, path.display());
    }
    Ok(())
}

fn cmd_ls_remote(args: &[String]) -> Result<(), String> {
    if args.len() > 2 {
        return Err("usage: cvm ls-remote [llvm|gcc] [prefix]".into());
    }
    let tools = if let Some(tool) = args.first() {
        vec![Tool::from_str(tool)?]
    } else {
        Tool::all().to_vec()
    };
    let prefix = args
        .get(1)
        .map(|value| VersionPrefix::parse(value))
        .transpose()?;

    for (idx, tool) in tools.iter().enumerate() {
        if idx > 0 {
            println!();
        }
        println!("{tool}:");
        let mut releases = remote_versions(*tool)?;
        if let Some(prefix) = &prefix {
            releases.retain(|release| release.version.matches_prefix(prefix));
        }
        if releases.is_empty() {
            println!("  <none>");
        } else {
            for release in releases {
                let date = release.date.unwrap_or_else(|| "-".to_string());
                println!("  {:<14} {}", release.version, date);
            }
        }
        println!("  compatibility: {}", compatibility_note(*tool));
    }
    Ok(())
}

fn cmd_list(args: &[String]) -> Result<(), String> {
    let tools = if args.is_empty() {
        Tool::all().to_vec()
    } else {
        vec![Tool::from_str(&args[0])?]
    };

    for tool in tools {
        println!("{tool}:");
        let mut versions = installed_versions(tool)?;
        versions.sort();
        let default = read_global_version(tool)?;
        if versions.is_empty() {
            println!("  <none>");
        } else {
            for version in versions {
                let marker = if default.as_deref() == Some(&version.to_string()) {
                    "default -> "
                } else {
                    ""
                };
                println!("  {marker}{version}");
            }
        }
    }
    Ok(())
}

fn cmd_use(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: cvm use <llvm|gcc|system> [version-or-prefix]".into());
    }
    if args[0] == "system" {
        if args.len() > 2 {
            return Err("usage: cvm use system [llvm|gcc]".into());
        }
        let tool = match args.get(1).map(String::as_str) {
            Some("llvm") => Some(Tool::Llvm),
            Some("gcc") => Some(Tool::Gcc),
            Some(_) => return Err("usage: cvm use system [llvm|gcc]".into()),
            None => None,
        };
        print!("{}", system_env_script(tool));
        return Ok(());
    }
    let tool = Tool::from_str(&args[0])?;
    let version = resolve_requested_version(tool, args.get(1).map(String::as_str))?;
    let prefix = install_prefix(tool, &version)?;
    ensure_installed(tool, &version, &prefix)?;
    print!("{}", env_script(tool, &prefix));
    Ok(())
}

fn cmd_env(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err(
            "usage: cvm env <llvm|gcc> [version-or-prefix] or cvm env <llvm@version|gcc@version>"
                .into(),
        );
    }

    if args[0] == "--defaults" {
        print!("{}", defaults_env_script()?);
        return Ok(());
    }

    let spec = if args[0].contains('@') {
        parse_tool_spec_request(&args[0])?
    } else {
        let tool = Tool::from_str(&args[0])?;
        let version = resolve_requested_version(tool, args.get(1).map(String::as_str))?;
        ToolSpec { tool, version }
    };

    let prefix = install_prefix(spec.tool, &spec.version)?;
    ensure_installed(spec.tool, &spec.version, &prefix)?;
    print!("{}", env_script(spec.tool, &prefix));
    Ok(())
}

fn cmd_deactivate(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("usage: cvm deactivate".into());
    }
    print!("{}", system_env_script(None));
    Ok(())
}

fn cmd_alias(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return list_aliases();
    }
    match args[0].as_str() {
        "default" => {
            if args.len() != 3 {
                return Err("usage: cvm alias default <llvm|gcc> <version-or-prefix>".into());
            }
            let tool = Tool::from_str(&args[1])?;
            let version = resolve_local_or_exact_version(tool, &args[2])?;
            let prefix = install_prefix(tool, &version)?;
            ensure_installed(tool, &version, &prefix)?;
            write_global_version(tool, &version)?;
            println!("default {tool} -> {version}");
            Ok(())
        }
        other => Err(format!(
            "unsupported alias: {other}; only default is supported"
        )),
    }
}

fn cmd_current(args: &[String]) -> Result<(), String> {
    let tools = if args.is_empty() {
        Tool::all().to_vec()
    } else {
        vec![Tool::from_str(&args[0])?]
    };

    for tool in tools {
        match read_global_version(tool)? {
            Some(version) => println!("{tool}: {version}"),
            None => println!("{tool}: <none>"),
        }
    }
    Ok(())
}

fn cmd_which(args: &[String]) -> Result<(), String> {
    if args.is_empty() || args.len() > 2 {
        return Err("usage: cvm which <llvm|gcc> [version-or-prefix]".into());
    }
    let tool = Tool::from_str(&args[0])?;
    let version = resolve_requested_version(tool, args.get(1).map(String::as_str))?;
    let prefix = install_prefix(tool, &version)?;
    ensure_installed(tool, &version, &prefix)?;
    let bin = prefix.join("bin");
    println!("{tool} {version}:");
    for name in which_binary_names(tool) {
        let path = bin.join(name);
        if path.exists() {
            println!("  {name}: {}", path.display());
        }
    }
    Ok(())
}

fn cmd_uninstall(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: cvm uninstall <llvm|gcc> <version-or-prefix>".into());
    }
    let tool = Tool::from_str(&args[0])?;
    let version = resolve_local_or_exact_version(tool, &args[1])?;
    let prefix = install_prefix(tool, &version)?;
    if !prefix.exists() {
        return Err(format!(
            "{tool} {version} is not installed at {}",
            prefix.display()
        ));
    }
    fs::remove_dir_all(&prefix)
        .map_err(|e| format!("failed to remove {}: {e}", prefix.display()))?;
    clear_global_version_if_matches(tool, &version)?;
    println!("removed {tool} {version}");
    Ok(())
}

fn cmd_upgrade(args: &[String]) -> Result<(), String> {
    let mut dry_run = false;
    let mut requested = None;
    for arg in args {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "-h" | "--help" => {
                println!("usage: cvm upgrade [version] [--dry-run]");
                return Ok(());
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown upgrade option: {value}"))
            }
            value => {
                if requested.is_some() {
                    return Err("usage: cvm upgrade [version] [--dry-run]".into());
                }
                requested = Some(value.to_string());
            }
        }
    }

    let tag = match requested {
        Some(version) => normalize_cvm_tag(&version)?,
        None => format!("v{}", latest_cvm_release()?),
    };
    let installer = format!("https://raw.githubusercontent.com/{CVM_REPO}/{tag}/install.sh");
    let asset = binary_asset_name()?;

    if dry_run {
        println!("upgrade: {tag}");
        println!("installer: {installer}");
        println!("asset: {asset}");
        return Ok(());
    }

    let body = fetch_text(&installer)?;
    let script = temporary_script_path("cvm-upgrade");
    fs::write(&script, body).map_err(|e| format!("failed to write {}: {e}", script.display()))?;
    let mut command = Command::new("bash");
    command.arg(&script).arg("--version").arg(&tag);
    let result = run_command(command);
    let _ = fs::remove_file(&script);
    result
}

fn cmd_init(args: &[String]) -> Result<(), String> {
    if !args.is_empty() {
        return Err("usage: cvm init".into());
    }

    let mut defaults = Vec::new();
    for tool in Tool::all() {
        if let Some(version) = read_global_version(tool)? {
            let version = Version::parse(&version)?;
            let prefix = install_prefix(tool, &version)?;
            defaults.push((tool, version, prefix));
        }
    }
    print!("{}", init_script(&defaults));
    Ok(())
}

fn cmd_completion(args: &[String]) -> Result<(), String> {
    if args.len() != 1 {
        return Err("usage: cvm completion <bash|zsh>".into());
    }
    match args[0].as_str() {
        "bash" => {
            print!("{}", bash_completion_script());
            Ok(())
        }
        "zsh" => {
            print!("{}", zsh_completion_script());
            Ok(())
        }
        _ => Err("usage: cvm completion <bash|zsh>".into()),
    }
}

fn bash_completion_script() -> String {
    format!(
        r#"# cvm bash completion
_cvm_installed_versions() {{
  local tool="$1"
  command cvm ls "$tool" 2>/dev/null | sed -n 's/^  default -> //p; t; s/^  \([^<].*\)$/\1/p'
}}

_cvm_complete() {{
  local cur command tool
  COMPREPLY=()
  cur="${{COMP_WORDS[COMP_CWORD]}}"
  command="${{COMP_WORDS[1]}}"

  local commands="{commands}"
  local tools="llvm gcc"

  if [ "$COMP_CWORD" -eq 1 ]; then
    COMPREPLY=( $(compgen -W "$commands" -- "$cur") )
    return 0
  fi

  case "$command" in
    completion)
      if [ "$COMP_CWORD" -eq 2 ]; then
        COMPREPLY=( $(compgen -W "bash zsh" -- "$cur") )
      fi
      ;;
    cache)
      if [ "$COMP_CWORD" -eq 2 ]; then
        COMPREPLY=( $(compgen -W "dir list prune" -- "$cur") )
      elif [ "${{COMP_WORDS[2]}}" = "prune" ]; then
        COMPREPLY=( $(compgen -W "--older-than" -- "$cur") )
      fi
      ;;
    profile)
      if [ "$COMP_CWORD" -eq 2 ]; then
        COMPREPLY=( $(compgen -W "template list" -- "$cur") )
      elif [ "${{COMP_WORDS[2]}}" = "template" ] && [ "$COMP_CWORD" -eq 3 ]; then
        COMPREPLY=( $(compgen -W "$tools" -- "$cur") )
      fi
      ;;
    install|ls-remote|ls|list|current)
      if [ "$COMP_CWORD" -eq 2 ]; then
        COMPREPLY=( $(compgen -W "$tools" -- "$cur") )
      fi
      ;;
    use)
      if [ "$COMP_CWORD" -eq 2 ]; then
        COMPREPLY=( $(compgen -W "$tools system" -- "$cur") )
      elif [ "$COMP_CWORD" -eq 3 ] && [ "${{COMP_WORDS[2]}}" = "system" ]; then
        COMPREPLY=( $(compgen -W "$tools" -- "$cur") )
      elif [ "$COMP_CWORD" -eq 3 ]; then
        tool="${{COMP_WORDS[2]}}"
        COMPREPLY=( $(compgen -W "$(_cvm_installed_versions "$tool")" -- "$cur") )
      fi
      ;;
    env|which|uninstall)
      if [ "$COMP_CWORD" -eq 2 ]; then
        COMPREPLY=( $(compgen -W "$tools" -- "$cur") )
      elif [ "$COMP_CWORD" -eq 3 ]; then
        tool="${{COMP_WORDS[2]}}"
        COMPREPLY=( $(compgen -W "$(_cvm_installed_versions "$tool")" -- "$cur") )
      fi
      ;;
    alias)
      if [ "$COMP_CWORD" -eq 2 ]; then
        COMPREPLY=( $(compgen -W "default" -- "$cur") )
      elif [ "$COMP_CWORD" -eq 3 ] && [ "${{COMP_WORDS[2]}}" = "default" ]; then
        COMPREPLY=( $(compgen -W "$tools" -- "$cur") )
      elif [ "$COMP_CWORD" -eq 4 ] && [ "${{COMP_WORDS[2]}}" = "default" ]; then
        tool="${{COMP_WORDS[3]}}"
        COMPREPLY=( $(compgen -W "$(_cvm_installed_versions "$tool")" -- "$cur") )
      fi
      ;;
  esac
  return 0
}}

if command -v complete >/dev/null 2>&1; then
  complete -F _cvm_complete cvm
fi
"#,
        commands = COMPLETION_COMMANDS
    )
}

fn zsh_completion_script() -> String {
    format!(
        r#"#compdef cvm
# cvm zsh completion

_cvm_installed_versions() {{
  local tool="$1"
  command cvm ls "$tool" 2>/dev/null | sed -n 's/^  default -> //p; t; s/^  \([^<].*\)$/\1/p'
}}

_cvm() {{
  local -a commands tools cache_commands profile_commands shells versions
  commands=({commands})
  tools=(llvm gcc)
  cache_commands=(dir list prune)
  profile_commands=(template list)
  shells=(bash zsh)

  if (( CURRENT == 2 )); then
    compadd -- $commands
    return
  fi

  case "$words[2]" in
    completion)
      if (( CURRENT == 3 )); then compadd -- $shells; fi
      ;;
    cache)
      if (( CURRENT == 3 )); then
        compadd -- $cache_commands
      elif [[ "$words[3]" == prune ]]; then
        compadd -- --older-than
      fi
      ;;
    profile)
      if (( CURRENT == 3 )); then
        compadd -- $profile_commands
      elif [[ "$words[3]" == template && CURRENT == 4 ]]; then
        compadd -- $tools
      fi
      ;;
    install|ls-remote|ls|list|current)
      if (( CURRENT == 3 )); then compadd -- $tools; fi
      ;;
    use)
      if (( CURRENT == 3 )); then
        compadd -- $tools system
      elif [[ "$words[3]" == system && CURRENT == 4 ]]; then
        compadd -- $tools
      elif (( CURRENT == 4 )); then
        versions=(${{(f)"$(_cvm_installed_versions "$words[3]")"}})
        compadd -- $versions
      fi
      ;;
    env|which|uninstall)
      if (( CURRENT == 3 )); then
        compadd -- $tools
      elif (( CURRENT == 4 )); then
        versions=(${{(f)"$(_cvm_installed_versions "$words[3]")"}})
        compadd -- $versions
      fi
      ;;
    alias)
      if (( CURRENT == 3 )); then
        compadd -- default
      elif [[ "$words[3]" == default && CURRENT == 4 ]]; then
        compadd -- $tools
      elif [[ "$words[3]" == default && CURRENT == 5 ]]; then
        versions=(${{(f)"$(_cvm_installed_versions "$words[4]")"}})
        compadd -- $versions
      fi
      ;;
  esac
}}

if whence -w compdef >/dev/null 2>&1; then
  compdef _cvm cvm
fi
"#,
        commands = COMPLETION_COMMANDS
    )
}

fn list_aliases() -> Result<(), String> {
    for tool in Tool::all() {
        match read_global_version(tool)? {
            Some(version) => println!("default {tool} -> {version}"),
            None => println!("default {tool} -> <none>"),
        }
    }
    Ok(())
}

fn build_profile_env(tool: Tool, path: &Path) -> Result<Vec<(String, String)>, String> {
    let body =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let profile = parse_build_profile(&body)?;
    match tool {
        Tool::Llvm => {
            let llvm = profile.llvm.ok_or_else(|| {
                format!(
                    "profile {} does not contain an [llvm] section",
                    path.display()
                )
            })?;
            llvm_profile_env(llvm)
        }
        Tool::Gcc => {
            let gcc = profile.gcc.ok_or_else(|| {
                format!(
                    "profile {} does not contain a [gcc] section",
                    path.display()
                )
            })?;
            gcc_profile_env(gcc)
        }
    }
}

fn parse_build_profile(input: &str) -> Result<BuildProfile, String> {
    toml::from_str(input).map_err(|e| format!("failed to parse build profile: {e}"))
}

fn default_build_profile_path(tool: Tool) -> Result<PathBuf, String> {
    Ok(cvm_home()?
        .join("profiles")
        .join("build")
        .join(tool.as_str())
        .join("default.toml"))
}

fn llvm_profile_env(profile: LlvmBuildProfile) -> Result<Vec<(String, String)>, String> {
    let mut envs = Vec::new();
    push_non_empty_env(&mut envs, "CVM_LLVM_TARGETS", profile.targets)?;
    push_non_empty_env(&mut envs, "CVM_LLVM_PROJECTS", profile.projects)?;
    push_non_empty_env(&mut envs, "CVM_LLVM_RUNTIMES", profile.runtimes)?;
    push_non_empty_env(&mut envs, "CVM_LLVM_BUILD_TYPE", profile.build_type)?;
    if let Some(defines) = profile.cmake_defines {
        let mut entries = Vec::new();
        for (key, value) in defines {
            if key.trim().is_empty() {
                return Err("llvm.cmake_defines contains an empty key".into());
            }
            reject_newlines("llvm.cmake_defines keys", &key)?;
            reject_newlines("llvm.cmake_defines values", &value)?;
            entries.push(format!("{key}={value}"));
        }
        if !entries.is_empty() {
            envs.push(("CVM_LLVM_CMAKE_DEFINES".into(), entries.join("\n")));
        }
    }
    Ok(envs)
}

fn gcc_profile_env(profile: GccBuildProfile) -> Result<Vec<(String, String)>, String> {
    let mut envs = Vec::new();
    push_non_empty_env(&mut envs, "CVM_GCC_LANGUAGES", profile.languages)?;
    if let Some(multilib) = profile.multilib {
        envs.push(("CVM_GCC_MULTILIB".into(), multilib.to_string()));
    }
    if let Some(bootstrap) = profile.bootstrap {
        envs.push(("CVM_GCC_BOOTSTRAP".into(), bootstrap.to_string()));
    }
    if let Some(args) = profile.configure_args {
        for arg in &args {
            if arg.trim().is_empty() {
                return Err("gcc.configure_args must not contain empty arguments".into());
            }
            reject_newlines("gcc.configure_args", arg)?;
        }
        if !args.is_empty() {
            envs.push(("CVM_GCC_CONFIGURE_ARGS".into(), args.join("\n")));
        }
    }
    Ok(envs)
}

fn push_non_empty_env(
    envs: &mut Vec<(String, String)>,
    key: &str,
    value: Option<String>,
) -> Result<(), String> {
    if let Some(value) = value {
        if value.trim().is_empty() {
            return Err(format!("{key} must not be empty"));
        }
        reject_newlines(key, &value)?;
        envs.push((key.into(), value));
    }
    Ok(())
}

fn reject_newlines(field: &str, value: &str) -> Result<(), String> {
    if value.contains('\n') || value.contains('\r') {
        return Err(format!("{field} must not contain newlines"));
    }
    Ok(())
}

fn profile_template(tool: Tool) -> &'static str {
    match tool {
        Tool::Llvm => LLVM_PROFILE_TEMPLATE,
        Tool::Gcc => GCC_PROFILE_TEMPLATE,
    }
}

fn resolve_remote_or_exact_install_target(
    tool: Tool,
    input: &str,
) -> Result<InstallTarget, String> {
    if let Ok(version) = Version::parse(input) {
        let source_url = default_source_url(tool, &version)?;
        return Ok(InstallTarget {
            version,
            source_url,
        });
    }
    let prefix = VersionPrefix::parse(input)?;
    remote_versions(tool)?
        .into_iter()
        .filter(|entry| entry.version.matches_prefix(&prefix))
        .max_by(|lhs, rhs| lhs.version.cmp(&rhs.version))
        .map(|entry| InstallTarget {
            version: entry.version,
            source_url: entry.url,
        })
        .ok_or_else(|| format!("no remote {tool} version matches prefix {input}"))
}

fn resolve_requested_version(tool: Tool, explicit: Option<&str>) -> Result<Version, String> {
    if let Some(version) = explicit {
        return resolve_local_or_exact_version(tool, version);
    }
    let version = read_global_version(tool)?
        .ok_or_else(|| format!("no default version configured for {tool}"))?;
    Version::parse(&version)
}

fn resolve_local_or_exact_version(tool: Tool, input: &str) -> Result<Version, String> {
    if let Ok(version) = Version::parse(input) {
        return Ok(version);
    }
    let prefix = VersionPrefix::parse(input)?;
    resolve_highest_matching_version(installed_versions(tool)?, &prefix)
        .ok_or_else(|| format!("no installed {tool} version matches prefix {input}"))
}

fn resolve_highest_matching_version<I>(versions: I, prefix: &VersionPrefix) -> Option<Version>
where
    I: IntoIterator<Item = Version>,
{
    versions
        .into_iter()
        .filter(|version| version.matches_prefix(prefix))
        .max()
}

fn maybe_alias_default_after_install(
    tool: Tool,
    version: &Version,
    using_custom_prefix: bool,
) -> Result<(), String> {
    if using_custom_prefix || read_global_version(tool)?.is_some() {
        return Ok(());
    }
    let versions = installed_versions(tool)?;
    if versions.len() == 1 && versions.first() == Some(version) {
        write_global_version(tool, version)?;
        println!("default {tool} -> {version}");
    }
    Ok(())
}

fn installed_versions(tool: Tool) -> Result<Vec<Version>, String> {
    let root = cvm_home()?.join("toolchains").join(tool.as_str());
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut versions = Vec::new();
    for entry in
        fs::read_dir(&root).map_err(|e| format!("failed to read {}: {e}", root.display()))?
    {
        let entry = entry.map_err(|e| format!("failed to read install entry: {e}"))?;
        if entry
            .file_type()
            .map_err(|e| format!("failed to inspect install entry: {e}"))?
            .is_dir()
        {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if let Ok(version) = Version::parse(&name) {
                versions.push(version);
            }
        }
    }
    Ok(versions)
}

fn default_source_url(tool: Tool, version: &Version) -> Result<String, String> {
    match tool {
        Tool::Gcc => Ok(format!(
            "https://ftp.gnu.org/gnu/gcc/gcc-{version}/gcc-{version}.tar.xz"
        )),
        Tool::Llvm => {
            let min_supported = Version::parse("9.0.1")?;
            let src_suffix = Version::parse("11.0.1")?;
            if version < &min_supported {
                return Err("LLVM versions older than 9.0.1 are not supported".into());
            }
            if version >= &src_suffix {
                Ok(format!(
                    "https://github.com/llvm/llvm-project/releases/download/llvmorg-{version}/llvm-project-{version}.src.tar.xz"
                ))
            } else {
                Ok(format!(
                    "https://github.com/llvm/llvm-project/releases/download/llvmorg-{version}/llvm-project-{version}.tar.xz"
                ))
            }
        }
    }
}

fn cache_root() -> Result<PathBuf, String> {
    Ok(cvm_home()?.join("cache"))
}

fn source_cache_root() -> Result<PathBuf, String> {
    Ok(cache_root()?.join("sources"))
}

fn source_cache_dir(tool: Tool, version: &Version) -> Result<PathBuf, String> {
    Ok(source_cache_root()?
        .join(tool.as_str())
        .join(version.to_string()))
}

fn key_cache_root() -> Result<PathBuf, String> {
    Ok(cache_root()?.join("keys"))
}

fn source_release_key_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Llvm => "release-keys.asc",
        Tool::Gcc => "gnu-keyring.gpg",
    }
}

fn source_release_key_url(tool: Tool) -> String {
    let env_key = match tool {
        Tool::Llvm => "CVM_LLVM_RELEASE_KEYS_URL",
        Tool::Gcc => "CVM_GCC_RELEASE_KEYS_URL",
    };
    env::var(env_key).unwrap_or_else(|_| {
        match tool {
            Tool::Llvm => LLVM_RELEASE_KEYS_URL,
            Tool::Gcc => GCC_RELEASE_KEYS_URL,
        }
        .to_string()
    })
}

fn ensure_source_release_keys(tool: Tool) -> Result<PathBuf, String> {
    let dir = key_cache_root()?.join(tool.as_str());
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create {}: {e}", dir.display()))?;
    let bundle = dir.join(source_release_key_name(tool));

    if bundle.is_file() {
        println!(
            "keys: using cached {tool} release key bundle {}",
            bundle.display()
        );
    } else {
        println!(
            "keys: missing {tool} release key bundle {}",
            bundle.display()
        );
        let url = source_release_key_url(tool);
        println!(
            "keys: downloading {tool} release key bundle to {}",
            bundle.display()
        );
        download_to_file(&url, &bundle)
            .map_err(|err| format!("failed to download {tool} release key bundle: {err}"))?;
    }

    println!(
        "keys: importing {tool} release key bundle {}",
        bundle.display()
    );
    let status = Command::new("gpg")
        .arg("--import")
        .arg(&bundle)
        .status()
        .map_err(|e| format!("failed to run gpg to import {tool} release keys: {e}"))?;
    if !status.success() {
        return Err(format!(
            "failed to import {tool} release key bundle {}",
            bundle.display()
        ));
    }

    Ok(bundle)
}

fn source_archive_name(url: &str) -> Result<String, String> {
    let path = url.strip_prefix("file://").unwrap_or(url);
    path.rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("failed to determine archive name from {url}"))
}

fn ensure_cached_source_package(
    tool: Tool,
    version: &Version,
    url: &str,
) -> Result<SourcePackage, String> {
    let dir = source_cache_dir(tool, version)?;
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create {}: {e}", dir.display()))?;
    let archive = dir.join(source_archive_name(url)?);
    let signature = archive.with_file_name(format!(
        "{}.sig",
        archive
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid archive name")?
    ));
    let downloaded_archive = if archive.is_file() {
        touch_cache_entry(&dir)?;
        println!(
            "cache: using {tool} {version} source archive {}",
            archive.display()
        );
        false
    } else {
        println!("cache: downloading {tool} {version} source archive");
        download_to_file(url, &archive)?;
        true
    };

    if downloaded_archive || !signature.is_file() {
        let signature_url = format!("{url}.sig");
        println!("cache: downloading {tool} {version} source signature");
        download_to_file(&signature_url, &signature)
            .map_err(|err| format!("failed to download source signature: {err}"))?;
    }
    touch_cache_entry(&dir)?;
    Ok(SourcePackage { archive, signature })
}

fn verify_source_signature(
    tool: Tool,
    version: &Version,
    source: &SourcePackage,
) -> Result<(), String> {
    let status = Command::new("gpg")
        .arg("--verify")
        .arg(&source.signature)
        .arg(&source.archive)
        .status()
        .map_err(|e| format!("failed to run gpg for source signature verification: {e}"))?;
    if !status.success() {
        return Err(format!(
            "source signature verification failed for {}",
            source.archive.display()
        ));
    }
    println!("verify: {tool} {version} source signature OK");
    Ok(())
}

fn download_to_file(url: &str, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create {}: {e}", parent.display()))?;
    }
    let partial = path.with_file_name(format!(
        ".{}.part",
        path.file_name()
            .and_then(|name| name.to_str())
            .ok_or("invalid download path")?
    ));
    match fs::remove_file(&partial) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("failed to remove {}: {err}", partial.display())),
    }

    if let Some(source) = url.strip_prefix("file://") {
        fs::copy(source, &partial).map_err(|e| format!("failed to copy {source}: {e}"))?;
    } else {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(300))
            .try_proxy_from_env(true)
            .build();
        let response = agent
            .get(url)
            .set("User-Agent", &format!("cvm/{CVM_VERSION}"))
            .call()
            .map_err(|e| describe_fetch_error(url, e))?;
        let mut reader = response.into_reader();
        let mut file = fs::File::create(&partial)
            .map_err(|e| format!("failed to create {}: {e}", partial.display()))?;
        io::copy(&mut reader, &mut file)
            .map_err(|e| format!("failed to write {}: {e}", partial.display()))?;
    }

    fs::rename(&partial, path).map_err(|e| {
        format!(
            "failed to move {} to {}: {e}",
            partial.display(),
            path.display()
        )
    })
}

fn touch_cache_entry(dir: &Path) -> Result<(), String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    fs::write(dir.join(".last-used"), format!("{now}\n"))
        .map_err(|e| format!("failed to update cache entry {}: {e}", dir.display()))
}

fn read_cache_last_used(dir: &Path) -> u64 {
    fs::read_to_string(dir.join(".last-used"))
        .ok()
        .and_then(|value| value.trim().parse::<u64>().ok())
        .or_else(|| {
            fs::metadata(dir)
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
        })
        .unwrap_or(0)
}

fn list_cache() -> Result<(), String> {
    let entries = cache_entries()?;
    if entries.is_empty() {
        println!(
            "no source archives found under {}",
            source_cache_root()?.display()
        );
        return Ok(());
    }
    println!("source archives:");
    for entry in entries {
        println!(
            "  {:<4} {:<14} {:>10} {}",
            entry.tool,
            entry.version,
            format_bytes(entry.bytes),
            entry.path.display()
        );
    }
    Ok(())
}

fn prune_cache_older_than(max_age_secs: u64, print: bool) -> Result<usize, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let mut pruned = 0;
    for entry in cache_entries()? {
        if now.saturating_sub(entry.last_used) <= max_age_secs {
            continue;
        }
        fs::remove_dir_all(&entry.dir)
            .map_err(|e| format!("failed to prune {}: {e}", entry.dir.display()))?;
        pruned += 1;
        if print {
            println!(
                "cache: pruned {} {} source archive",
                entry.tool, entry.version
            );
        }
    }
    Ok(pruned)
}

fn cache_entries() -> Result<Vec<CacheEntry>, String> {
    let root = source_cache_root()?;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for tool in Tool::all() {
        let tool_dir = root.join(tool.as_str());
        let read_dir = match fs::read_dir(&tool_dir) {
            Ok(read_dir) => read_dir,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(format!("failed to read {}: {err}", tool_dir.display())),
        };
        for entry in read_dir {
            let entry =
                entry.map_err(|err| format!("failed to read {}: {err}", tool_dir.display()))?;
            if !entry
                .file_type()
                .map_err(|err| format!("failed to inspect cache entry: {err}"))?
                .is_dir()
            {
                continue;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Ok(version) = Version::parse(&name) else {
                continue;
            };
            let dir = entry.path();
            let Some((path, bytes)) = cache_archive_file(&dir)? else {
                continue;
            };
            entries.push(CacheEntry {
                tool,
                version,
                dir: dir.clone(),
                path,
                bytes,
                last_used: read_cache_last_used(&dir),
            });
        }
    }
    entries.sort_by(|left, right| {
        left.tool
            .as_str()
            .cmp(right.tool.as_str())
            .then_with(|| right.version.cmp(&left.version))
    });
    Ok(entries)
}

fn cache_archive_file(dir: &Path) -> Result<Option<(PathBuf, u64)>, String> {
    let mut archives = Vec::new();
    for entry in fs::read_dir(dir).map_err(|e| format!("failed to read {}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("failed to read {}: {e}", dir.display()))?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.ends_with(".tar.xz") {
            let bytes = entry
                .metadata()
                .map_err(|e| format!("failed to stat {}: {e}", path.display()))?
                .len();
            archives.push((path, bytes));
        }
    }
    archives.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(archives.into_iter().next())
}

struct CacheEntry {
    tool: Tool,
    version: Version,
    dir: PathBuf,
    path: PathBuf,
    bytes: u64,
    last_used: u64,
}

fn parse_duration_arg(input: &str) -> Result<u64, String> {
    let trimmed = input.trim();
    let digits_len = trimmed
        .trim_end_matches(|c: char| c.is_ascii_alphabetic())
        .len();
    let (digits, unit) = trimmed.split_at(digits_len);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("invalid duration: {input}"));
    }
    let value = digits
        .parse::<u64>()
        .map_err(|_| format!("duration is too large: {input}"))?;
    let multiplier = match unit {
        "" | "s" => 1,
        "m" => 60,
        "h" => 60 * 60,
        "d" => 24 * 60 * 60,
        _ => return Err(format!("unsupported duration unit: {input}")),
    };
    value
        .checked_mul(multiplier)
        .ok_or_else(|| format!("duration is too large: {input}"))
}

fn format_bytes(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = KIB * 1024;
    const GIB: u64 = MIB * 1024;
    if bytes >= GIB {
        format!("{:.1} GiB", bytes as f64 / GIB as f64)
    } else if bytes >= MIB {
        format!("{:.1} MiB", bytes as f64 / MIB as f64)
    } else if bytes >= KIB {
        format!("{:.1} KiB", bytes as f64 / KIB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn remote_versions(tool: Tool) -> Result<Vec<RemoteVersion>, String> {
    let index = load_remote_index()?;
    Ok(remote_versions_from_index(&index, tool))
}

fn latest_cvm_release() -> Result<Version, String> {
    let index = load_remote_index()?;
    parse_cvm_tag(&index.cvm.latest)
}

fn load_remote_index() -> Result<RemoteIndex, String> {
    let url = env::var("CVM_REMOTE_INDEX_URL").unwrap_or_else(|_| DEFAULT_REMOTE_INDEX_URL.into());
    let body = if url == "builtin" {
        DEFAULT_REMOTE_INDEX.to_string()
    } else {
        fetch_text(&url)?
    };
    parse_remote_index(&body)
}

fn parse_remote_index(input: &str) -> Result<RemoteIndex, String> {
    let index: RemoteIndex =
        serde_json::from_str(input).map_err(|e| format!("failed to parse remote index: {e}"))?;
    if index.schema_version != 1 {
        return Err(format!(
            "unsupported remote index schema version: {}",
            index.schema_version
        ));
    }
    Ok(index)
}

fn remote_versions_from_index(index: &RemoteIndex, tool: Tool) -> Vec<RemoteVersion> {
    let entries = match tool {
        Tool::Llvm => &index.compilers.llvm,
        Tool::Gcc => &index.compilers.gcc,
    };
    let mut versions: Vec<RemoteVersion> = entries
        .iter()
        .filter_map(|entry| {
            let version = Version::parse(&entry.version).ok()?;
            Some(RemoteVersion {
                version,
                date: Some(entry.date.clone()),
                url: entry.url.clone(),
            })
        })
        .collect();
    versions.sort_by(|lhs, rhs| rhs.version.cmp(&lhs.version));
    versions.dedup_by(|lhs, rhs| lhs.version == rhs.version);
    versions
}

fn fetch_text(url: &str) -> Result<String, String> {
    if let Some(path) = url.strip_prefix("file://") {
        return fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"));
    }

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(5))
        .try_proxy_from_env(true)
        .build();
    let request = agent
        .get(url)
        .set("User-Agent", &format!("cvm/{CVM_VERSION}"));

    request
        .call()
        .map_err(|e| describe_fetch_error(url, e))?
        .into_string()
        .map_err(|e| format!("failed to read response from {url}: {e}"))
}

fn describe_fetch_error(url: &str, err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, response) => {
            let body = response.into_string().unwrap_or_default();
            let mut message = format!("failed to fetch {url}: status code {code}");
            let body = body.trim();
            if !body.is_empty() {
                message.push_str("; ");
                message.push_str(&body.chars().take(240).collect::<String>());
            }
            message
        }
        other => format!("failed to fetch {url}: {other}"),
    }
}

fn compatibility_note(tool: Tool) -> String {
    let platform = binary_platform_name().unwrap_or_else(|err| format!("unsupported ({err})"));
    match tool {
        Tool::Gcc => format!(
            "{platform}; source builds use GNU GCC releases and Debian/Ubuntu apt dependency bootstrap"
        ),
        Tool::Llvm => format!(
            "{platform}; source builds support LLVM >= 9.0.1 and Debian/Ubuntu apt dependency bootstrap"
        ),
    }
}

fn print_version_diagnostics() {
    println!("diagnostics:");
    let home = match cvm_home() {
        Ok(home) => {
            println!("  CVM_HOME: {}", home.display());
            home
        }
        Err(err) => {
            println!("  CVM_HOME: <unavailable: {err}>");
            return;
        }
    };

    match env::current_exe() {
        Ok(path) => println!("  cvm binary: {}", path.display()),
        Err(err) => println!("  cvm binary: <unknown: {err}>"),
    }

    let loader = home.join("cvm.sh");
    println!(
        "  cvm.sh: {}",
        if loader.is_file() { "found" } else { "missing" }
    );

    let cvm_bin = home.join("bin");
    println!(
        "  PATH: {}",
        if path_contains(&cvm_bin) {
            "$CVM_HOME/bin found"
        } else {
            "$CVM_HOME/bin missing"
        }
    );

    for tool in Tool::all() {
        print_default_diagnostic(&home, tool);
    }
}

fn print_default_diagnostic(home: &Path, tool: Tool) {
    let label = format!("default {tool}");
    match read_global_version(tool) {
        Ok(Some(version)) => match Version::parse(&version) {
            Ok(version) => {
                let prefix = install_prefix_for_home(home, tool, &version);
                let status = if prefix.join("bin").is_dir() {
                    "installed"
                } else {
                    "missing"
                };
                println!("  {label}: {version} ({status})");
            }
            Err(err) => println!("  {label}: {version} (invalid: {err})"),
        },
        Ok(None) => println!("  {label}: <none>"),
        Err(err) => println!("  {label}: <error: {err}>"),
    }
}

fn path_contains(path: &Path) -> bool {
    env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).any(|entry| entry == path))
        .unwrap_or(false)
}

fn which_binary_names(tool: Tool) -> &'static [&'static str] {
    match tool {
        Tool::Llvm => &[
            "clang",
            "clang++",
            "ld.lld",
            "llvm-ar",
            "llvm-nm",
            "llvm-objcopy",
        ],
        Tool::Gcc => &["gcc", "g++", "gcov"],
    }
}

fn binary_asset_name() -> Result<String, String> {
    Ok(format!("cvm-{}.tar.gz", binary_platform_name()?))
}

fn binary_platform_name() -> Result<String, String> {
    let os = match env::consts::OS {
        "linux" => "unknown-linux-musl",
        "macos" => "apple-darwin",
        other => return Err(format!("unsupported OS: {other}")),
    };
    let arch = match env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        other => return Err(format!("unsupported arch: {other}")),
    };
    Ok(format!("{arch}-{os}"))
}

fn normalize_cvm_tag(input: &str) -> Result<String, String> {
    parse_cvm_tag(input)?;
    let version = input.strip_prefix('v').unwrap_or(input);
    Ok(format!("v{version}"))
}

fn parse_cvm_tag(input: &str) -> Result<Version, String> {
    Version::parse(input.strip_prefix('v').unwrap_or(input))
}

fn temporary_script_path(prefix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    env::temp_dir().join(format!("{prefix}-{}-{stamp}.sh", std::process::id()))
}

fn ensure_installed(tool: Tool, version: &Version, prefix: &Path) -> Result<(), String> {
    if prefix.join("bin").is_dir() {
        Ok(())
    } else {
        Err(format!(
            "{tool} {version} is not installed at {}",
            prefix.display()
        ))
    }
}

fn install_prefix(tool: Tool, version: &Version) -> Result<PathBuf, String> {
    Ok(install_prefix_for_home(&cvm_home()?, tool, version))
}

fn cvm_home() -> Result<PathBuf, String> {
    cvm_home_from_env(
        env::var_os("CVM_HOME").as_ref(),
        env::var_os("HOME").as_ref(),
    )
}

fn write_global_version(tool: Tool, version: &Version) -> Result<(), String> {
    let defaults = cvm_home()?.join("defaults");
    fs::create_dir_all(&defaults).map_err(|e| format!("failed to create defaults dir: {e}"))?;
    fs::write(defaults.join(tool.as_str()), version.to_string())
        .map_err(|e| format!("failed to write global default: {e}"))
}

fn clear_global_version_if_matches(tool: Tool, version: &Version) -> Result<(), String> {
    let path = cvm_home()?.join("defaults").join(tool.as_str());
    let Some(current) = read_global_version(tool)? else {
        return Ok(());
    };
    if current == version.to_string() {
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("failed to remove {}: {e}", path.display())),
        }
    }
    Ok(())
}

fn read_global_version(tool: Tool) -> Result<Option<String>, String> {
    let path = cvm_home()?.join("defaults").join(tool.as_str());
    match fs::read_to_string(&path) {
        Ok(s) => Ok(Some(s.trim().to_string())),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("failed to read {}: {e}", path.display())),
    }
}

fn ensure_build_script(tool: Tool) -> Result<PathBuf, String> {
    let scripts_dir = cvm_home()?.join("scripts");
    fs::create_dir_all(&scripts_dir).map_err(|e| format!("failed to create scripts dir: {e}"))?;
    let (name, body) = match tool {
        Tool::Llvm => ("build_llvm-project.sh", LLVM_BUILD_SCRIPT),
        Tool::Gcc => ("build_gcc.sh", GCC_BUILD_SCRIPT),
    };
    let path = scripts_dir.join(name);
    fs::write(&path, body).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    make_executable(&path)?;
    Ok(path)
}

fn defaults_env_script() -> Result<String, String> {
    let mut defaults = Vec::new();
    for tool in Tool::all() {
        if let Some(version) = read_global_version(tool)? {
            let version = Version::parse(&version)?;
            let prefix = install_prefix(tool, &version)?;
            ensure_installed(tool, &version, &prefix)?;
            defaults.push(prefix.join("bin"));
        }
    }

    if defaults.is_empty() {
        return Ok(String::new());
    }

    let mut script = strip_toolchain_paths_script(None);
    for bin in defaults {
        script.push_str(&format!(
            "export PATH=\"{}:$PATH\"\n",
            shell_escape_path(&bin)
        ));
    }
    Ok(script)
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .map_err(|e| format!("failed to stat {}: {e}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|e| format!("failed to chmod {}: {e}", path.display()))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn run_command(mut command: Command) -> Result<(), String> {
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|e| format!("failed to start command: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("command exited with {status}"))
    }
}

fn shell_escape_path(path: &Path) -> String {
    path.as_os_str()
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn strip_toolchain_paths_script(tool: Option<Tool>) -> String {
    let path_pattern = match tool {
        Some(Tool::Llvm) => r#""${CVM_HOME:-$HOME/.cvm}"/toolchains/llvm/*/bin"#,
        Some(Tool::Gcc) => r#""${CVM_HOME:-$HOME/.cvm}"/toolchains/gcc/*/bin"#,
        None => r#""${CVM_HOME:-$HOME/.cvm}"/toolchains/*/*/bin"#,
    };
    r#"_cvm_strip_toolchain_paths() {
  _cvm_old_path="${PATH:-}"
  _cvm_new_path=""
  while [ -n "$_cvm_old_path" ]; do
    case "$_cvm_old_path" in
      *:*)
        _cvm_entry="${_cvm_old_path%%:*}"
        _cvm_old_path="${_cvm_old_path#*:}"
        ;;
      *)
        _cvm_entry="$_cvm_old_path"
        _cvm_old_path=""
        ;;
    esac
    case "$_cvm_entry" in
      __CVM_TOOLCHAIN_PATH_PATTERN__) ;;
      *)
        if [ -z "$_cvm_new_path" ]; then
          _cvm_new_path="$_cvm_entry"
        else
          _cvm_new_path="$_cvm_new_path:$_cvm_entry"
        fi
        ;;
    esac
  done
  PATH="$_cvm_new_path"
  export PATH
  unset _cvm_old_path _cvm_new_path _cvm_entry
}
_cvm_strip_toolchain_paths
"#
    .replace("__CVM_TOOLCHAIN_PATH_PATTERN__", path_pattern)
}

fn parse_u32(value: &str, original: &str) -> Result<u32, String> {
    if value.is_empty() || !value.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("invalid numeric component in version: {original}"));
    }
    value
        .parse::<u32>()
        .map_err(|_| format!("version component is too large: {original}"))
}

fn print_help() {
    let mut stdout = io::stdout();
    let _ = writeln!(
        stdout,
        "cvm {CVM_VERSION} - compiler version manager\n\n\
         Usage:\n\
           cvm install <llvm|gcc> <version-or-prefix> [-jN|--jobs N] [--profile PATH] [--targets LIST]\n\
           cvm cache <dir|list|prune>\n\
           cvm profile template <llvm|gcc> [PATH] [--force]\n\
           cvm profile list\n\
           cvm ls-remote [llvm|gcc] [prefix]\n\
           cvm ls [llvm|gcc]\n\
           cvm use <llvm|gcc|system> [version-or-prefix]\n\
           cvm alias default <llvm|gcc> <version-or-prefix>\n\
           cvm current [llvm|gcc]\n\
           cvm env <llvm|gcc> [version-or-prefix]\n\
           cvm which <llvm|gcc> [version-or-prefix]\n\
           cvm uninstall <llvm|gcc> <version-or-prefix>\n\
           cvm deactivate\n\
           cvm upgrade [version] [--dry-run]\n\
           cvm init\n\
           cvm version\n\n\
         Examples:\n\
           cvm install llvm 21 -j8\n\
           cvm cache dir\n\
           cvm cache list\n\
           cvm cache prune --older-than 14d\n\
           cvm profile template llvm\n\
           cvm profile list\n\
           cvm install llvm 21 --profile ./llvm-custom.toml\n\
           cvm ls-remote llvm 21\n\
           eval \"$(cvm use llvm 21)\"\n\
           cvm use system\n\
           cvm deactivate\n\
           cvm which llvm\n\
           cvm alias default llvm 21.1.8\n\
           cvm upgrade --dry-run\n\
           eval \"$(cvm init)\"\n"
    );
}

#[derive(Default)]
struct InstallOptions {
    jobs: Option<String>,
    targets: Option<String>,
    profile: Option<PathBuf>,
    prefix: Option<PathBuf>,
    force_configure: bool,
    dry_run: bool,
}

const LLVM_PROFILE_TEMPLATE: &str = r#"# cvm LLVM build profile
#
# This template mirrors cvm's default kernel-oriented LLVM build.
# Generate it, edit only the fields you need, then install normally:
#
#   cvm install llvm 21
#
# The default LLVM build profile lives at:
#
#   $CVM_HOME/profiles/build/llvm/default.toml

[llvm]

# LLVM targets to build. X86 is enough for common x86 Linux kernel builds.
# Add AArch64, ARM, RISCV, etc. when you need cross-kernel work.
targets = "X86"

# Keep clang and lld for Linux kernel builds. compiler-rt is useful for
# sanitizer/runtime work.
projects = "clang;lld;compiler-rt"

# Runtime libraries used by the default cvm LLVM build.
runtimes = "libcxx;libcxxabi;libunwind"

# Release is the default. Debug builds are much larger and slower.
build_type = "Release"

# Extra -DKEY=VALUE definitions passed to CMake.
[llvm.cmake_defines]
# LLVM_ENABLE_ASSERTIONS = "ON"
# LLVM_ENABLE_ZSTD = "ON"
"#;

const GCC_PROFILE_TEMPLATE: &str = r#"# cvm GCC build profile
#
# This template mirrors cvm's default kernel-oriented GCC build.
# Generate it, edit only the fields you need, then install normally:
#
#   cvm install gcc 15
#
# The default GCC build profile lives at:
#
#   $CVM_HOME/profiles/build/gcc/default.toml

[gcc]

# GCC frontend languages. The default C/C++ set is enough for Linux kernel
# builds and normal C/C++ development.
languages = "c,c++"

# false keeps the default --disable-multilib behavior.
multilib = false

# false keeps the default --disable-bootstrap behavior for faster builds.
bootstrap = false

# Extra configure arguments appended after cvm's defaults.
configure_args = [
  # "--enable-plugin",
  # "--enable-lto",
]
"#;
