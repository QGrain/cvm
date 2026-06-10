#!/usr/bin/env python3
"""Synchronize cvm's compiler remote index."""

from __future__ import annotations

import argparse
import datetime as dt
import html
import json
import re
import sys
import tomllib
import urllib.request
from pathlib import Path
from urllib.parse import urljoin


GCC_INDEX_URL = "https://ftp.gnu.org/gnu/gcc/"
LLVM_RELEASES_URL = "https://github.com/llvm/llvm-project/releases"
LLVM_MIN_VERSION = "9.0.1"


def version_key(version: str) -> tuple[int, int, int, int, int]:
    core, rc = (version.split("-rc", 1) + [""])[:2] if "-rc" in version else (version, "")
    major, minor, patch = (int(part) for part in core.split("."))
    rc_rank = 0 if rc else 1
    rc_value = int(rc) if rc else 0
    return major, minor, patch, rc_rank, rc_value


def fetch_text(url: str) -> str:
    request = urllib.request.Request(url, headers={"User-Agent": "cvm-index-sync/0.0.2"})
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read().decode("utf-8", errors="replace")


def gcc_url(version: str) -> str:
    return f"https://ftp.gnu.org/gnu/gcc/gcc-{version}/gcc-{version}.tar.xz"


def llvm_url(version: str) -> str:
    suffix = "src.tar.xz" if version_key(version) >= version_key("11.0.1") else "tar.xz"
    return (
        "https://github.com/llvm/llvm-project/releases/download/"
        f"llvmorg-{version}/llvm-project-{version}.{suffix}"
    )


def parse_gcc_index(body: str) -> list[dict[str, str]]:
    entries: list[dict[str, str]] = []
    seen: set[str] = set()
    pattern = re.compile(r'href="gcc-(\d+\.\d+\.\d+)/".*?(\d{4}-\d{2}-\d{2})')
    for match in pattern.finditer(body):
        version, date = match.groups()
        if version in seen:
            continue
        seen.add(version)
        entries.append({"version": version, "date": date, "url": gcc_url(version)})
    return sorted(entries, key=lambda entry: version_key(entry["version"]), reverse=True)


def extract_datetime_near(body: str, start: int) -> str | None:
    window = body[max(0, start - 2500) : start + 2500]
    matches = re.findall(r'datetime="([^"]+)"', window)
    if not matches:
        return None
    return matches[0][:10]


def parse_llvm_releases_page(body: str) -> tuple[list[dict[str, str]], str | None]:
    entries: list[dict[str, str]] = []
    seen: set[str] = set()
    pattern = re.compile(r'/llvm/llvm-project/releases/tag/llvmorg-([0-9]+\.[0-9]+\.[0-9]+(?:-rc[0-9]+)?)')
    for match in pattern.finditer(body):
        version = html.unescape(match.group(1))
        if version in seen or version_key(version) < version_key(LLVM_MIN_VERSION):
            continue
        date = extract_datetime_near(body, match.start())
        if date is None:
            continue
        seen.add(version)
        entries.append({"version": version, "date": date, "url": llvm_url(version)})

    next_url = None
    next_match = re.search(r'<a[^>]+href="([^"]+)"[^>]+rel="next"', body)
    if next_match:
        next_url = urljoin(LLVM_RELEASES_URL, html.unescape(next_match.group(1)))
    return entries, next_url


def fetch_llvm_releases(start_url: str) -> list[dict[str, str]]:
    entries: dict[str, dict[str, str]] = {}
    url: str | None = start_url
    while url:
        page_entries, url = parse_llvm_releases_page(fetch_text(url))
        for entry in page_entries:
            entries.setdefault(entry["version"], entry)
    return sorted(entries.values(), key=lambda entry: version_key(entry["version"]), reverse=True)


def cvm_latest(repo_root: Path) -> str:
    manifest = tomllib.loads((repo_root / "Cargo.toml").read_text())
    return f"v{manifest['package']['version']}"


def build_index(repo_root: Path, gcc_index_url: str, llvm_releases_url: str) -> dict[str, object]:
    return {
        "schema_version": 1,
        "generated_at": dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
        "cvm": {"latest": cvm_latest(repo_root)},
        "compilers": {
            "gcc": parse_gcc_index(fetch_text(gcc_index_url)),
            "llvm": fetch_llvm_releases(llvm_releases_url),
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", default="manifests/remote-index.json")
    parser.add_argument("--gcc-index-url", default=GCC_INDEX_URL)
    parser.add_argument("--llvm-releases-url", default=LLVM_RELEASES_URL)
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[1]
    index = build_index(repo_root, args.gcc_index_url, args.llvm_releases_url)
    output = repo_root / args.output
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(index, indent=2, sort_keys=False) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
