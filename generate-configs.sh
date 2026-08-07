#!/bin/bash
# Pattern adapted from Chromium's third_party/libvpx generate_gni.sh (BSD-licensed):
# https://chromium.googlesource.com/webm/libvpx/+/main/generate_gni.sh
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

if [ ! -d libvpx ]; then
  echo "error: ./libvpx submodule not found (run: git submodule update --init)" >&2
  exit 1
fi

libvpx_dir="$(pwd)/libvpx"
targets=(
  x86_64-linux-gcc
  arm64-linux-gcc
  x86_64-win64-vs17
  x86_64-darwin20-gcc
  arm64-darwin20-gcc
  arm64-darwin-gcc
  x86_64-iphonesimulator-gcc
)

scratch="$(mktemp -d "$(pwd)/.gen-configs.XXXXXX")"
trap 'rm -rf "$scratch"' EXIT

for target in "${targets[@]}"; do
  echo "=== $target ==="
  build_dir="$scratch/$target"
  mkdir -p "$build_dir"
  out_dir="configs/$target"
  rm -rf "$out_dir"
  mkdir -p "$out_dir"

  configure_args=(
    --target="$target"
    --enable-external-build
    --disable-vp8
    --disable-examples
    --disable-tools
    --disable-docs
    --disable-unit-tests
    --enable-pic
    --as=yasm
  )
  # external-build auto-enables SVE, which Apple toolchains reject.
  case "$target" in
    x86_64-darwin20-gcc | arm64-darwin20-gcc | arm64-darwin-gcc | x86_64-iphonesimulator-gcc)
      configure_args+=(--disable-sve --disable-sve2)
      ;;
  esac

  (
    cd "$build_dir"
    "$libvpx_dir/configure" "${configure_args[@]}"
    make libvpx_srcs.txt vpx_config.asm vp9_rtcd.h vpx_dsp_rtcd.h vpx_scale_rtcd.h
  )

  cp "$build_dir"/vpx_config.h \
     "$build_dir"/vpx_config.c \
     "$build_dir"/vpx_config.asm \
     "$build_dir"/vpx_version.h \
     "$build_dir"/vp9_rtcd.h \
     "$build_dir"/vpx_dsp_rtcd.h \
     "$build_dir"/vpx_scale_rtcd.h \
     "$build_dir"/libvpx_srcs.txt \
     "$out_dir/"
done
