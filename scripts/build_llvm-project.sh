#!/usr/bin/env bash
set -Eeuo pipefail

usage() {
	cat <<'EOF'
Usage: build_llvm-project.sh LLVM_VERSION [options]

Build and install llvm-project from source into a versioned prefix.

Options:
  -j, --jobs N              Ninja parallelism (default: half of nproc, minimum 1)
      --prefix DIR          Install prefix (default: $PWD/llvm-project-${VERSION}.install)
      --targets LIST        LLVM targets (default: X86)
      --force-configure     Remove an existing build directory before CMake
  -h, --help                Show this help

Examples:
  ./build_llvm-project.sh 21.1.8
      Build LLVM 21.1.8 with default X86 kernel-oriented tools.

  ./build_llvm-project.sh 21.1.8 -j8
      Build with 8 parallel Ninja jobs.

  ./build_llvm-project.sh 21.1.8 --prefix "$HOME/toolchains/llvm-21.1.8"
      Install into an explicit prefix for manual PATH management or cvm.

  ./build_llvm-project.sh 21.1.8 --targets "X86;AArch64"
      Build extra LLVM backends when reproducing non-x86 kernel builds.
EOF
}

die() {
	echo "build_llvm-project.sh: $*" >&2
	exit 1
}

on_error() {
	echo "build_llvm-project.sh: failed at line $1" >&2
}
trap 'on_error $LINENO' ERR

version_ge() {
	local lhs=$1 rhs=$2
	local lhs_core=${lhs%%-rc*} rhs_core=${rhs%%-rc*}
	local IFS=.
	local -a l r
	read -r -a l <<<"$lhs_core"
	read -r -a r <<<"$rhs_core"
	for i in 0 1 2; do
		local lv=${l[$i]:-0} rv=${r[$i]:-0}
		if ((lv > rv)); then return 0; fi
		if ((lv < rv)); then return 1; fi
	done
	return 0
}

validate_version() {
	[[ $1 =~ ^[0-9]+\.[0-9]+\.[0-9]+(-rc[0-9]+)?$ ]] || die "version must be X.Y.Z or X.Y.Z-rcN: $1"
}

llvm_url() {
	local version=$1
	if version_ge "$version" "11.0.1"; then
		echo "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/llvm-project-${version}.src.tar.xz"
	elif version_ge "$version" "9.0.1"; then
		echo "https://github.com/llvm/llvm-project/releases/download/llvmorg-${version}/llvm-project-${version}.tar.xz"
	else
		die "LLVM versions older than 9.0.1 are not supported"
	fi
}

jobs=""
prefix=""
targets="X86"
projects="clang;lld;compiler-rt"
runtimes="libcxx;libcxxabi;libunwind"
build_type="Release"
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
	--targets)
		shift
		[[ $# -gt 0 ]] || die "--targets requires a value"
		targets=$1
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

sudo apt update
sudo apt install -y cmake ninja-build libedit-dev python3-dev swig wget xz-utils

cwd=$(pwd -P)
prefix=${prefix:-"${cwd}/llvm-project-${version}.install"}
url=$(llvm_url "$version")
archive=$(basename "$url")
src_dir=${archive%.tar.xz}
build_dir="${src_dir}/build"

if [[ ! -f $archive ]]; then
	wget -O "$archive" "$url"
fi

if [[ ! -d $src_dir ]]; then
	tar -xJf "$archive"
fi

if ((force_configure)); then
	rm -rf "$build_dir"
fi
mkdir -p "$build_dir" "$prefix"

cmake_args=(
	-G Ninja
	-DCMAKE_BUILD_TYPE="${build_type}"
	-DLLVM_ENABLE_PROJECTS="${projects}"
	-DLLVM_TARGETS_TO_BUILD="${targets}"
	-DLLVM_INSTALL_UTILS=ON
	-DCMAKE_INSTALL_PREFIX="${prefix}"
)
cmake_args+=(-DLLVM_ENABLE_RUNTIMES="${runtimes}")

cmake -S "${src_dir}/llvm" -B "$build_dir" "${cmake_args[@]}"
ninja -C "$build_dir" -j"$jobs"
ninja -C "$build_dir" install

for tool in clang clang++ ld.lld llvm-ar llvm-nm llvm-objcopy llvm-objdump llvm-readelf llvm-strip; do
	[[ -x "${prefix}/bin/${tool}" ]] || die "missing installed tool: ${prefix}/bin/${tool}"
done

rm -rf "$src_dir" "$archive"

cat <<EOF
Installed LLVM ${version} to ${prefix}
For Linux kernel builds:
  export PATH="${prefix}/bin:\$PATH"
  make LLVM=${prefix}/bin/
EOF
