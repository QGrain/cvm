use std::cmp::Ordering;
use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::str::FromStr;

pub const CVM_VERSION: &str = env!("CARGO_PKG_VERSION");

const LLVM_BUILD_SCRIPT: &str = include_str!("../scripts/build_llvm-project.sh");
const GCC_BUILD_SCRIPT: &str = include_str!("../scripts/build_gcc.sh");

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

pub fn parse_tool_spec(input: &str) -> Result<ToolSpec, String> {
    let (tool, version) = input
        .split_once('@')
        .ok_or_else(|| format!("tool spec must look like llvm@21.1.8 or gcc@15.1.0: {input}"))?;
    Ok(ToolSpec {
        tool: Tool::from_str(tool)?,
        version: Version::parse(version)?,
    })
}

pub fn env_script(tool: Tool, prefix: &Path) -> String {
    let bin = prefix.join("bin");
    let bin = shell_escape_path(&bin);
    let strip = strip_toolchain_paths_script();
    match tool {
        Tool::Llvm => format!(
            "{strip}{}{}",
            managed_env_reset_script(),
            tool_env_exports(tool, &bin)
        ),
        Tool::Gcc => format!(
            "{strip}{}{}",
            managed_env_reset_script(),
            tool_env_exports(tool, &bin)
        ),
    }
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
"#,
    );
    script
}

pub fn resolve_active_version(local: Option<&str>, global: Option<&str>) -> Option<String> {
    local.or(global).map(ToString::to_string)
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
        "version" | "--version" | "-V" => {
            println!("cvm {CVM_VERSION}");
            Ok(())
        }
        "install" => cmd_install(&rest),
        "ls" | "list" => cmd_list(&rest),
        "use" => cmd_use(&rest),
        "env" => cmd_env(&rest),
        "alias" => cmd_alias(&rest),
        "current" => cmd_current(&rest),
        "uninstall" => cmd_uninstall(&rest),
        "init" => cmd_init(&rest),
        other => Err(format!("unknown command: {other}")),
    }
}

fn cmd_install(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: cvm install <llvm|gcc> <version> [-jN|--jobs N] [--targets LIST] [--prefix DIR] [--dry-run]".into());
    }

    let tool = Tool::from_str(&args[0])?;
    let version = Version::parse(&args[1])?;
    let mut options = InstallOptions::default();
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

    if options.dry_run {
        println!("bash {}", command_args.join(" "));
        return Ok(());
    }

    let mut command = Command::new("bash");
    command.args(command_args);
    run_command(command)
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
        return Err("usage: cvm use <llvm|gcc> [version]".into());
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
            "usage: cvm env <llvm|gcc> [version] or cvm env <llvm@version|gcc@version>".into(),
        );
    }

    if args[0] == "--defaults" {
        print!("{}", defaults_env_script()?);
        return Ok(());
    }

    let spec = if args[0].contains('@') {
        parse_tool_spec(&args[0])?
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

fn cmd_alias(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return list_aliases();
    }
    match args[0].as_str() {
        "default" => {
            if args.len() != 3 {
                return Err("usage: cvm alias default <llvm|gcc> <version>".into());
            }
            let tool = Tool::from_str(&args[1])?;
            let version = Version::parse(&args[2])?;
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
        let local = read_local_version(tool)?;
        let global = read_global_version(tool)?;
        match resolve_active_version(local.as_deref(), global.as_deref()) {
            Some(version) => println!("{tool}: {version}"),
            None => println!("{tool}: <none>"),
        }
    }
    Ok(())
}

fn cmd_uninstall(args: &[String]) -> Result<(), String> {
    if args.len() < 2 {
        return Err("usage: cvm uninstall <llvm|gcc> <version>".into());
    }
    let tool = Tool::from_str(&args[0])?;
    let version = Version::parse(&args[1])?;
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

fn list_aliases() -> Result<(), String> {
    for tool in Tool::all() {
        match read_global_version(tool)? {
            Some(version) => println!("default {tool} -> {version}"),
            None => println!("default {tool} -> <none>"),
        }
    }
    Ok(())
}

fn resolve_requested_version(tool: Tool, explicit: Option<&str>) -> Result<Version, String> {
    if let Some(version) = explicit {
        return Version::parse(version);
    }
    let local = read_local_version(tool)?;
    let global = read_global_version(tool)?;
    let chosen = resolve_active_version(local.as_deref(), global.as_deref())
        .ok_or_else(|| format!("no active version configured for {tool}"))?;
    Version::parse(&chosen)
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

fn read_local_version(tool: Tool) -> Result<Option<String>, String> {
    let mut dir = env::current_dir().map_err(|e| format!("failed to read current dir: {e}"))?;
    loop {
        let path = dir.join(".cvmrc");
        if path.exists() {
            for (entry_tool, version) in read_cvmrc(&path)? {
                if entry_tool == tool {
                    return Ok(Some(version));
                }
            }
        }
        if !dir.pop() {
            return Ok(None);
        }
    }
}

fn read_cvmrc(path: &Path) -> Result<Vec<(Tool, String)>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("failed to read {}: {e}", path.display())),
    };

    let mut entries = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (tool, version) = if let Some((tool, version)) = line.split_once('=') {
            (tool.trim(), version.trim())
        } else if let Some((tool, version)) = line.split_once('@') {
            (tool.trim(), version.trim())
        } else {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() != 2 {
                return Err(format!("invalid .cvmrc line: {line}"));
            }
            (parts[0], parts[1])
        };
        Version::parse(version)?;
        entries.push((Tool::from_str(tool)?, version.to_string()));
    }
    Ok(entries)
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
            defaults.push((tool, prefix.join("bin")));
        }
    }

    if defaults.is_empty() {
        return Ok(String::new());
    }

    let mut script = strip_toolchain_paths_script().to_string();
    script.push_str(managed_env_reset_script());
    for (tool, bin) in defaults {
        script.push_str(&tool_env_exports(tool, &shell_escape_path(&bin)));
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

fn strip_toolchain_paths_script() -> &'static str {
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
      "${CVM_HOME:-$HOME/.cvm}"/toolchains/*/*/bin) ;;
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
}

fn managed_env_reset_script() -> &'static str {
    "unset CC CXX LD LLVM HOSTCC HOSTCXX\n"
}

fn tool_env_exports(tool: Tool, bin: &str) -> String {
    match tool {
        Tool::Llvm => format!(
            "export PATH=\"{bin}:$PATH\"\nexport LLVM=\"{bin}/\"\nexport CC=\"clang\"\nexport CXX=\"clang++\"\nexport LD=\"ld.lld\"\n"
        ),
        Tool::Gcc => format!(
            "export PATH=\"{bin}:$PATH\"\nexport CC=\"gcc\"\nexport CXX=\"g++\"\nexport HOSTCC=\"gcc\"\nexport HOSTCXX=\"g++\"\n"
        ),
    }
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
           cvm install <llvm|gcc> <version> [-jN|--jobs N]\n\
           cvm ls [llvm|gcc]\n\
           cvm use <llvm|gcc> [version]\n\
           cvm alias default <llvm|gcc> <version>\n\
           cvm current [llvm|gcc]\n\
           cvm env <llvm|gcc> [version]\n\
           cvm uninstall <llvm|gcc> <version>\n\
           cvm init\n\
           cvm version\n\n\
         Examples:\n\
           cvm install llvm 21.1.8 -j8\n\
           eval \"$(cvm use llvm 21.1.8)\"\n\
           cvm alias default llvm 21.1.8\n\
           eval \"$(cvm init)\"\n"
    );
}

#[derive(Default)]
struct InstallOptions {
    jobs: Option<String>,
    targets: Option<String>,
    prefix: Option<PathBuf>,
    force_configure: bool,
    dry_run: bool,
}
