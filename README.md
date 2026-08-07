# libvpx-sys

Rust bindings to [libvpx](https://github.com/webmproject/libvpx), pinned to
v1.16.0 and vendored as a git submodule. Scope is VP9 only (VP8 is disabled
at configure time); bindings are allowlisted to `v*`/`V*` symbols, so the
VP9 encoder/decoder interfaces (`vpx_codec_vp9_cx`, `vpx_codec_vp9_dx`) and
their control IDs are exposed but VP8 is not.

## Versioning

The crate version carries the vendored libvpx version as build metadata:
`0.1.0+libvpx-1.16.0`. Bumping libvpx means:

1. Update the `libvpx` submodule to the new tag.
2. Bump the `+libvpx-x.y.z` suffix in `Cargo.toml`.
3. Re-run `generate-configs.sh` and `regen-bindings.sh`, and commit the
   updated `configs/` and `src/bindings.rs`.

## System dependencies

### Build (consuming the crate)

- A C compiler toolchain.
- nasm >= 2.09, x86/x86_64 targets only (assembles libvpx's `.asm` sources;
  arm64 targets need no assembler).
- MSVC targets: `cl.exe` plus nasm.
- The `lto` feature additionally requires clang and a bitcode-capable
  linker (lld; mold cannot consume LLVM bitcode).

### Dev (regenerating vendored artifacts)

- `generate-configs.sh`: perl, make, and awk. It runs entirely on a Linux
  host and produces configs for all 7 supported targets; no cross
  toolchains are required.
- `regen-bindings.sh`: `bindgen-cli` (installed automatically via
  `cargo install bindgen-cli` if missing) and libclang.

## Layout

- `libvpx/` — the vendored libvpx submodule (v1.16.0).
- `configs/<target>/` — per-target `vpx_config.h`/`.c`/`.asm`,
  `*_rtcd.h`, and `libvpx_srcs.txt`, committed so consumers don't need
  libvpx's own configure/make toolchain.
- `ffi.h` — the public libvpx headers exposed to bindgen.
- `src/bindings.rs` — committed bindgen output.

## License

`libvpx-sys` is covered by the license in `./LICENSE` (this repository).
The vendored libvpx sources are BSD-3-Clause with an additional patent
grant — see `libvpx/LICENSE` and `libvpx/PATENTS`, which travel with any
redistribution of this crate.
