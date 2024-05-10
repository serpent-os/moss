#!/usr/bin/env bash
set -euxo pipefail

# Script to generate a tarball of source code and vendored (downloaded) Rust dependencies
# and the cargo configuration to ensure they are used

# Get the current directory, which we'll use for telling Cargo where to find the sources
wd="$PWD"

# Get the version from git-describe
#VERSION=$(git describe --dirty 2>/dev/null)
VERSION="0.10"

# The path where we will output the tar file
path=$wd/moss-$VERSION-vendored.tar.zst

# Clean up stuff we've written before
rm -f "$path"

# Make sure cargo lock files are in sync with cargo.toml
cargo check --locked

PREFIX_TMPDIR=$(mktemp -d)
pushd "$PREFIX_TMPDIR"

# Enable dotglob so we copy over files/folders starting with .
shopt -s dotglob
cp -ra "$wd"/* .

function get_commit_time() {
  TZ=UTC0 git log -1 \
    --format=tformat:%cd \
    --date=format:%Y-%m-%dT%H:%M:%SZ \
    "$@"
}

# Set each file mtime to that of it's latest commit
# Set each source file timestamp to that of its latest commit.
git ls-files | while read -r file; do
  commit_time=$(get_commit_time "$file") &&
  touch -md "$commit_time" "$file"
done

# Set timestamp of each directory under $FILES
# to the latest timestamp of any descendant.
find . -depth -type d -exec sh -c \
  'touch -r "$0/$(ls -At "$0" | head -n 1)" "$0"' \
  {} ';'

SOURCE_EPOCH=$(get_commit_time)

# Cleanup repo
git reset --hard
git clean -xdf
git clean -df
rm -rf .git
rm -rf serpent-style

# Generate vendored dependencies and the configuration to use them
mkdir .cargo
cargo vendor --manifest-path "$wd/Cargo.toml" > .cargo/config

# vendoring drags in a lot of Windows dependencies, which makes the resulting tarball enormous
# cargo can't be told only to support a particular platform
# see https://github.com/rust-lang/cargo/issues/7058
# workaround below from https://github.com/rust-lang/cargo/issues/7058#issuecomment-751856262
rm -r vendor/winapi*/lib/*.a

# Reproducible tar flags
TARFLAGS="
  --sort=name --format=posix
  --pax-option=exthdr.name=%d/PaxHeaders/%f
  --pax-option=delete=atime,delete=ctime
  --clamp-mtime --mtime=$SOURCE_EPOCH
  --numeric-owner --owner=0 --group=0
  --mode=go+u,go-w
"
ZSTDFLAGS="-19 -T0"

# shellcheck disable=SC2086
LC_ALL=C tar $TARFLAGS -C $PREFIX_TMPDIR -cf  - . |
  zstd $ZSTDFLAGS > $path

popd
rm -rf "$PREFIX_TMPDIR"

checksum=$(sha256sum "$path")
echo "Release tar checksum $checksum"
