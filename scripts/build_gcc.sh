#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
	cat <<'EOF'
Usage: build_gcc.sh GCC_VERSION [options]

Build and install GCC from source into a versioned prefix.

Options:
  -j, --jobs N              Make parallelism (default: half of nproc, minimum 1)
      --prefix DIR          Install prefix (default: $PWD/gcc-${VERSION}.install)
      --force-configure     Remove an existing build directory before configure
  -h, --help                Show this help

Environment overrides used by cvm build profiles:
  CVM_GCC_LANGUAGES          Configure languages (default: c,c++)
  CVM_GCC_MULTILIB           true enables multilib, false disables it
  CVM_GCC_BOOTSTRAP          true enables bootstrap, false disables it
  CVM_GCC_CONFIGURE_ARGS     Newline-separated extra configure arguments

Examples:
  ./build_gcc.sh 15.1.0
      Build GCC 15.1.0 with default kernel-oriented C/C++ support.

  ./build_gcc.sh 15.1.0 -j8
      Build with 8 parallel Make jobs.

  ./build_gcc.sh 15.1.0 --prefix "$HOME/toolchains/gcc-15.1.0"
      Install into an explicit prefix for manual PATH management or cvm.
EOF
}

die() {
	echo "build_gcc.sh: $*" >&2
	exit 1
}

on_error() {
	echo "build_gcc.sh: failed at line $1" >&2
}
trap 'on_error $LINENO' ERR

validate_version() {
	[[ $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die "version must be X.Y.Z: $1"
}

jobs=""
prefix=""
mirror="https://ftp.gnu.org/gnu/gcc"
build_dir=""
force_configure=0
version=""

while (($#)); do
	case "$1" in
	-j)
		shift
		[[ $# -gt 0 ]] || die "-j requires a value"
		jobs=$1
		;;
	-j*)
		jobs=${1#-j}
		;;
	--jobs)
		shift
		[[ $# -gt 0 ]] || die "--jobs requires a value"
		jobs=$1
		;;
	--prefix)
		shift
		[[ $# -gt 0 ]] || die "--prefix requires a value"
		prefix=$1
		;;
	--force-configure)
		force_configure=1
		;;
	-h | --help)
		usage
		exit 0
		;;
	-*)
		die "unknown option: $1"
		;;
	*)
		[[ -z $version ]] || die "multiple versions provided: $version and $1"
		version=$1
		;;
	esac
	shift
done

[[ -n $version ]] || {
	usage
	exit 1
}
validate_version "$version"

[[ -z $jobs || $jobs =~ ^[0-9]+$ ]] || die "jobs must be numeric: $jobs"
if [[ -z $jobs ]]; then
	nproc_value=$(nproc 2>/dev/null || echo 2)
	jobs=$((nproc_value / 2))
	((jobs >= 1)) || jobs=1
fi
gcc_languages="${CVM_GCC_LANGUAGES:-c,c++}"
gcc_multilib="${CVM_GCC_MULTILIB:-false}"
gcc_bootstrap="${CVM_GCC_BOOTSTRAP:-false}"
[[ -n $gcc_languages ]] || die "CVM_GCC_LANGUAGES must not be empty"
[[ $gcc_multilib == "true" || $gcc_multilib == "false" ]] || die "CVM_GCC_MULTILIB must be true or false"
[[ $gcc_bootstrap == "true" || $gcc_bootstrap == "false" ]] || die "CVM_GCC_BOOTSTRAP must be true or false"

sudo apt update
sudo apt install -y build-essential flex bison texinfo wget xz-utils

cwd=$(pwd -P)
src_dir="gcc-${version}"
prefix=${prefix:-"${cwd}/gcc-${version}.install"}
build_dir=${build_dir:-"${src_dir}/build"}
archive="${src_dir}.tar.xz"
url="${mirror}/gcc-${version}/${archive}"
configure_script="${cwd}/${src_dir}/configure"

if [[ ! -f $archive ]]; then
	wget -O "$archive" "$url"
fi

if [[ ! -d $src_dir ]]; then
	tar -xJf "$archive"
fi

(
	cd "$src_dir"
	./contrib/download_prerequisites
)

if ((force_configure)); then
	rm -rf "$build_dir"
fi
mkdir -p "$build_dir" "$prefix"

(
	cd "$build_dir"
	configure_args=(
		"--prefix=$prefix"
		"--enable-languages=$gcc_languages"
	)
	if [[ $gcc_multilib == "false" ]]; then
		configure_args+=(--disable-multilib)
	fi
	if [[ $gcc_bootstrap == "false" ]]; then
		configure_args+=(--disable-bootstrap)
	fi
	if [[ -n ${CVM_GCC_CONFIGURE_ARGS:-} ]]; then
		while IFS= read -r arg; do
			[[ -n $arg ]] || continue
			configure_args+=("$arg")
		done <<<"$CVM_GCC_CONFIGURE_ARGS"
	fi
	"$configure_script" "${configure_args[@]}"
	make -j"$jobs"
	make install
)

for tool in gcc g++ cpp gcov; do
	[[ -x "${prefix}/bin/${tool}" ]] || die "missing installed tool: ${prefix}/bin/${tool}"
done

rm -rf "$src_dir" "$archive"

cat <<EOF
Installed GCC ${version} to ${prefix}
For Linux kernel builds:
  export PATH="${prefix}/bin:\$PATH"
  make CC=gcc HOSTCC=gcc
EOF
