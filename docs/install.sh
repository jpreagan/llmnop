#!/bin/sh

set -eu

installer_url="https://github.com/jpreagan/llmnop/releases/latest/download/llmnop-installer.sh"
tmp_dir="$(mktemp -d "${TMPDIR:-/tmp}/llmnop.XXXXXX")"
installer="$tmp_dir/llmnop-installer.sh"

cleanup() {
    rm -rf "$tmp_dir"
}
trap cleanup 0 HUP INT TERM

curl --proto '=https' --tlsv1.2 -sSfL "$installer_url" -o "$installer"
sh "$installer" "$@"
