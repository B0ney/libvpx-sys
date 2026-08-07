#!/bin/bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

# Pinned: bindings.rs is committed, and bindgen output differs across versions.
bindgen_version="0.72.1"
if ! bindgen --version 2>/dev/null | grep -qF "bindgen $bindgen_version"; then
  echo "error: bindgen $bindgen_version required." >&2
  echo "install with: cargo install bindgen-cli --version $bindgen_version --locked" >&2
  exit 1
fi

bindgen ffi.h \
  --allowlist-type '^[vV].*' \
  --allowlist-var '^[vV].*' \
  --allowlist-function '^[vV].*' \
  --rustified-enum '^v.*' \
  --no-layout-tests \
  --no-doc-comments \
  --distrust-clang-mangling \
  -o src/bindings.rs \
  -- \
  -I libvpx
