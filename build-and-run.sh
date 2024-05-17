#!/usr/bin/env bash

set -euo pipefail

target_dir="${CARGO_TARGET_DIR:-./target}"
mkdir -p "$target_dir/bin"

cargo build --bins

declare -a bins=(parent child grandchild)

for bin in "${bins[@]}"; do
    cp -f "$target_dir/debug/$bin" "$target_dir/bin/"
done

export PATH="$target_dir/bin:$PATH"

exec parent spawn-self
