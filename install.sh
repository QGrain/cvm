#!/usr/bin/env bash
set -Eeuo pipefail

repo="${CVM_REPO:-QGrain/cvm}"
cvm_latest_version() {
	printf '%s\n' "v0.0.8"
}

version="${CVM_VERSION:-$(cvm_latest_version)}"
cvm_home="${CVM_HOME:-${HOME}/.cvm}"
install_dir="${cvm_home}/bin"
script_dir=""

usage() {
	cat <<EOF
Usage: install.sh [--version VERSION]

Environment:
  CVM_REPO     GitHub repo, default: ${repo}
  CVM_VERSION  Release tag, default: $(cvm_latest_version)
  CVM_HOME     Install and data root, default: ${cvm_home}
  PROFILE      Shell profile to update, or /dev/null to skip profile edits
EOF
}

has() {
	command -v "$1" >/dev/null 2>&1
}

download() {
	local url=$1
	local output=$2

	if has curl; then
		curl -fsSL "$url" -o "$output"
	elif has wget; then
		wget -q "$url" -O "$output"
	else
		echo "install.sh: curl or wget is required" >&2
		return 1
	fi
}

require_command() {
	if ! has "$1"; then
		echo "install.sh: $1 is required" >&2
		return 1
	fi
}

resolve_script_dir() {
	local source="${BASH_SOURCE[0]:-$0}"
	[[ -n $source && -f $source ]] || return 1
	(
		cd -P "$(dirname "$source")" >/dev/null 2>&1
		pwd
	)
}

try_profile() {
	[[ -n ${1-} && -f $1 ]] || return 1
	printf '%s\n' "$1"
}

detect_profile() {
	if [[ ${PROFILE-} == "/dev/null" ]]; then
		return 0
	fi

	if [[ -n ${PROFILE-} && -f ${PROFILE} ]]; then
		printf '%s\n' "$PROFILE"
		return 0
	fi

	local detected=""
	if [[ ${SHELL-} == *bash* ]]; then
		if [[ -f "$HOME/.bashrc" ]]; then
			detected="$HOME/.bashrc"
		elif [[ -f "$HOME/.bash_profile" ]]; then
			detected="$HOME/.bash_profile"
		fi
	elif [[ ${SHELL-} == *zsh* ]]; then
		if [[ -f "${ZDOTDIR:-${HOME}}/.zshrc" ]]; then
			detected="${ZDOTDIR:-${HOME}}/.zshrc"
		elif [[ -f "${ZDOTDIR:-${HOME}}/.zprofile" ]]; then
			detected="${ZDOTDIR:-${HOME}}/.zprofile"
		fi
	fi

	if [[ -z $detected ]]; then
		local each
		for each in ".profile" ".bashrc" ".bash_profile" ".zprofile" ".zshrc"; do
			if detected="$(try_profile "${ZDOTDIR:-${HOME}}/${each}")"; then
				break
			fi
		done
	fi

	[[ -n $detected ]] && printf '%s\n' "$detected"
}

append_profile_snippet() {
	local profile
	profile="$(detect_profile)"
	local profile_cvm_home
	profile_cvm_home="$(printf '%s\n' "$cvm_home" | sed "s:^${HOME}:\$HOME:")"
	local source_str
	source_str=$(cat <<EOF

export CVM_HOME="${profile_cvm_home}"
[ -s "\$CVM_HOME/cvm.sh" ] && . "\$CVM_HOME/cvm.sh"  # This loads cvm
EOF
)

	if [[ ${PROFILE-} == "/dev/null" ]]; then
		printf 'Skipping profile update because PROFILE=/dev/null\n'
		return 0
	fi

	if [[ -z $profile ]]; then
		printf 'Profile not found. Append the following lines to your shell profile:\n'
		printf '%s\n' "$source_str"
		return 0
	fi

	if grep -qc '\$CVM_HOME/cvm\.sh' "$profile"; then
		printf 'cvm source string already in %s\n' "$profile"
	else
		printf 'Appending cvm source string to %s\n' "$profile"
		printf '%s\n' "$source_str" >>"$profile"
	fi
}

generate_shell_loader() {
	CVM_HOME="$cvm_home" "${install_dir}/cvm" init >"${cvm_home}/cvm.sh"
}

finish_install() {
	generate_shell_loader
	append_profile_snippet

	cat <<EOF
cvm installed to ${install_dir}/cvm

Open a new shell or run:
  export CVM_HOME="${cvm_home}"
  . "\$CVM_HOME/cvm.sh"
EOF
}

install_binary() {
	local binary=$1
	mkdir -p "$install_dir" "$cvm_home"
	install -m 0755 "$binary" "${install_dir}/cvm"
	finish_install
}

is_local_checkout() {
	[[ -n $script_dir ]] || return 1
	[[ -f "${script_dir}/Cargo.toml" ]] || return 1
	[[ -f "${script_dir}/src/main.rs" ]] || return 1
	[[ -d "${script_dir}/scripts" ]] || return 1
	grep -q '^name = "cvm"' "${script_dir}/Cargo.toml"
}

install_from_local_checkout() {
	require_command cargo
	printf 'Installing cvm from local checkout: %s\n' "$script_dir"
	(
		cd "$script_dir"
		cargo build --release
	)
	install_binary "${script_dir}/target/release/cvm"
}

while (($#)); do
	case "$1" in
	--version)
		shift
		[[ $# -gt 0 ]] || {
			echo "install.sh: --version requires a value" >&2
			exit 1
		}
		version=$1
		;;
	-h | --help)
		usage
		exit 0
		;;
	*)
		echo "install.sh: unknown option: $1" >&2
		exit 1
		;;
	esac
	shift
done

script_dir="$(resolve_script_dir || true)"

case "$(uname -s)" in
Linux) os=unknown-linux-musl ;;
Darwin) os=apple-darwin ;;
*) echo "install.sh: unsupported OS: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
x86_64 | amd64) arch=x86_64 ;;
aarch64 | arm64) arch=aarch64 ;;
*) echo "install.sh: unsupported arch: $(uname -m)" >&2; exit 1 ;;
esac

install_from_binary_asset() {
	local tmp=$1
	local name="cvm-${arch}-${os}.tar.gz"
	local url="https://github.com/${repo}/releases/download/${version}/${name}"

	printf 'Downloading cvm binary asset: %s\n' "$url"
	download "$url" "${tmp}/${name}" || return 1
	tar -xzf "${tmp}/${name}" -C "$tmp" || return 1
	[[ -x "${tmp}/cvm" ]] || {
		echo "install.sh: binary asset does not contain executable cvm at archive root" >&2
		return 1
	}
	install_binary "${tmp}/cvm"
}

find_source_root() {
	local tmp=$1
	local manifest
	manifest="$(find "$tmp" -mindepth 2 -maxdepth 3 -name Cargo.toml -print -quit)"
	[[ -n $manifest ]] || return 1
	dirname "$manifest"
}

install_from_source_archive() {
	local tmp=$1
	local source="${tmp}/source.tar.gz"
	local url="https://github.com/${repo}/archive/refs/tags/${version}.tar.gz"
	local root

	require_command cargo
	printf 'Downloading cvm source archive: %s\n' "$url"
	download "$url" "$source"
	tar -xzf "$source" -C "$tmp"
	root="$(find_source_root "$tmp")" || {
		echo "install.sh: source archive does not contain Cargo.toml" >&2
		return 1
	}
	printf 'Building cvm from source archive: %s\n' "$root"
	(
		cd "$root"
		cargo build --release
	)
	install_binary "${root}/target/release/cvm"
}

if is_local_checkout; then
	install_from_local_checkout
else
	tmp=$(mktemp -d)
	trap 'rm -rf "$tmp"' EXIT
	if ! install_from_binary_asset "$tmp"; then
		printf 'Binary asset unavailable; falling back to source archive.\n'
		install_from_source_archive "$tmp"
	fi
fi
