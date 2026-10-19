//! Compiles the vendored libvpx (see `libvpx/`) into a static `vpx` archive
//! using the per-target sources and headers committed under `configs/`.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let target = env::var("TARGET").expect("TARGET must be set by cargo");
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let is_arm = target_arch == "aarch64";
    let is_msvc = target_env == "msvc";
    let lto = env::var_os("CARGO_FEATURE_LTO").is_some();
    if lto && env::var("CARGO_CFG_TARGET_OS").unwrap() != "linux" {
        panic!("the `lto` feature currently supports only Linux targets");
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let config_dir = manifest_dir.join("configs").join(config_dir_name(&target));
    let libvpx_dir = manifest_dir.join("libvpx");
    if !libvpx_dir.join("configure").exists() {
        panic!(
            "libvpx submodule missing at {}; run: git submodule update --init",
            libvpx_dir.display()
        );
    }

    validate_version(&config_dir);
    println!("cargo:rerun-if-changed={}", config_dir.display());

    let (mut c_files, asm_files) = read_srcs(&config_dir, &libvpx_dir);
    c_files.push(config_dir.join("vpx_config.c"));

    let mut objects = compile_c(&c_files, &config_dir, &libvpx_dir, is_arm, is_msvc, lto);
    if !asm_files.is_empty() {
        // nasm output is native ELF, not bitcode; lld links it into the LTO
        // archive unmodified alongside the clang-emitted bitcode objects.
        objects.extend(compile_asm(&asm_files, &config_dir, &libvpx_dir));
    }

    let mut archive = cc::Build::new();
    for object in &objects {
        archive.object(object);
    }
    archive.compile("vpx");
}

fn config_dir_name(target: &str) -> &'static str {
    match target {
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gcc",
        "aarch64-unknown-linux-gnu" => "arm64-linux-gcc",
        "x86_64-pc-windows-msvc" => "x86_64-win64-vs17",
        "x86_64-pc-windows-gnu" => "x86_64-win64-gcc",
        "i686-pc-windows-gnu" => "x86-win32-gcc",
        "i686-pc-windows-msvc" => "x86-win32-vs17",
        "x86_64-apple-darwin" => "x86_64-darwin20-gcc",
        "aarch64-apple-darwin" => "arm64-darwin20-gcc",
        "aarch64-apple-ios" | "aarch64-apple-ios-sim" => "arm64-darwin-gcc",
        "x86_64-apple-ios" => "x86_64-iphonesimulator-gcc",
        other => panic!(
            "libvpx-sys has no vendored config for target `{other}`. Supported: \
             x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, x86_64-pc-windows-msvc, \
             x86_64-pc-windows-gnu, x86_64-apple-darwin, aarch64-apple-darwin, aarch64-apple-ios, \
             aarch64-apple-ios-sim, x86_64-apple-ios. To add one, see \
             libvpx-sys/generate-configs.sh."
        ),
    }
}

fn validate_version(config_dir: &Path) {
    let crate_version = env::var("CARGO_PKG_VERSION").unwrap();
    let Some((_, meta)) = crate_version.split_once('+') else {
        panic!(
            "CARGO_PKG_VERSION `{crate_version}` is missing the `+libvpx-x.y.z` build \
             metadata that pins the vendored libvpx version"
        );
    };
    let Some(declared) = meta.strip_prefix("libvpx-") else {
        panic!("expected build metadata `libvpx-x.y.z`, got `{meta}`");
    };

    let header = read(&config_dir.join("vpx_version.h"));
    let field = |name: &str| -> &str {
        header
            .lines()
            .find_map(|l| l.strip_prefix(&format!("#define {name}")))
            .unwrap_or_else(|| panic!("{name} not found in vpx_version.h"))
            .trim()
    };
    let vendored = format!(
        "{}.{}.{}",
        field("VERSION_MAJOR"),
        field("VERSION_MINOR"),
        field("VERSION_PATCH")
    );

    if declared != vendored {
        panic!(
            "crate version declares libvpx-{declared} but the vendored copy at \
             {} is {vendored}",
            config_dir.display()
        );
    }
}

/// Splits `libvpx_srcs.txt` entries under `libvpx_dir` into `.c` and `.asm` paths.
fn read_srcs(config_dir: &Path, libvpx_dir: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let list = read(&config_dir.join("libvpx_srcs.txt"));
    let mut c_files = Vec::new();
    let mut asm_files = Vec::new();
    for line in list.lines() {
        let line = line.trim();
        if line == "vpx_config.c" {
            // Named as build-generated in libs.mk; the caller pushes the
            // per-target copy committed alongside this config instead.
            continue;
        } else if line.ends_with(".c") {
            c_files.push(libvpx_dir.join(line));
        } else if line.ends_with(".asm") {
            asm_files.push(libvpx_dir.join(line));
        } else if [".S", ".s", ".cc", ".cpp"]
            .iter()
            .any(|e| line.ends_with(e))
        {
            panic!("libvpx_srcs.txt lists `{line}`, a source kind build.rs does not compile");
        }
    }
    (c_files, asm_files)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// libvpx dispatches SIMD kernels by file name suffix and expects each one
/// compiled with the matching arch flag; mirrors build/make/Makefile.
fn simd_flags(stem: &str, is_arm: bool, is_msvc: bool) -> Vec<&'static str> {
    if is_arm {
        if stem.ends_with("_sve2") {
            vec!["-march=armv9-a+i8mm+sve2"]
        } else if stem.ends_with("_sve") {
            vec!["-march=armv8.2-a+dotprod+i8mm+sve"]
        } else if stem.ends_with("_neon_i8mm") {
            vec!["-march=armv8.2-a+dotprod+i8mm"]
        } else if stem.ends_with("_neon_dotprod") {
            vec!["-march=armv8.2-a+dotprod"]
        } else {
            vec![]
        }
    } else if is_msvc {
        if stem.ends_with("_avx512") {
            vec!["/arch:AVX512"]
        } else if stem.ends_with("_avx2") {
            vec!["/arch:AVX2"]
        } else if stem.ends_with("_avx") {
            vec!["/arch:AVX"]
        } else {
            vec![] // sse2/ssse3/sse4 need no flag: x64 MSVC baseline already has them
        }
    } else if stem.ends_with("_avx512") {
        vec![
            "-mavx512f",
            "-mavx512cd",
            "-mavx512bw",
            "-mavx512dq",
            "-mavx512vl",
        ]
    } else if stem.ends_with("_avx2") {
        vec!["-mavx2"]
    } else if stem.ends_with("_avx") {
        vec!["-mavx"]
    } else if stem.ends_with("_sse4") {
        vec!["-msse4.1"]
    } else if stem.ends_with("_ssse3") {
        vec!["-mssse3"]
    } else if stem.ends_with("_sse2") {
        vec!["-msse2"]
    } else {
        // fixes compilation errors with mmintrin.h when compiling to i686-windows
        vec!["-msse2"]
    }
}

/// Compiles each SIMD flag group with its own `cc::Build` so an arch flag
/// never leaks onto a file that doesn't request it.
fn compile_c(
    files: &[PathBuf],
    config_dir: &Path,
    libvpx_dir: &Path,
    is_arm: bool,
    is_msvc: bool,
    lto: bool,
) -> Vec<PathBuf> {
    let mut groups: BTreeMap<Vec<&'static str>, Vec<&PathBuf>> = BTreeMap::new();
    for file in files {
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        groups
            .entry(simd_flags(stem, is_arm, is_msvc))
            .or_default()
            .push(file);
    }

    let mut objects = Vec::new();
    for (flags, group_files) in groups {
        let mut build = cc::Build::new();
        build
            .include(config_dir)
            .include(libvpx_dir)
            .warnings(false);
        if !is_msvc {
            build.flag("-pthread");
        }
        if lto {
            // WHY: cc-rs has a dormant linker-plugin-lto path that injects
            // `-flto=thin` when `-Clinker-plugin-lto` appears in RUSTFLAGS;
            // if that ever activates it will conflict with the flag below.
            // This block owns `-flto=full` deliberately.
            build.compiler("clang").flag("-flto=full");
        }
        for flag in flags {
            build.flag(flag);
        }
        build.files(group_files);
        objects.extend(build.compile_intermediates());
    }
    objects
}

fn compile_asm(files: &[PathBuf], config_dir: &Path, libvpx_dir: &Path) -> Vec<PathBuf> {
    let mut build = nasm_rs::Build::new();
    build.include(config_dir).include(libvpx_dir);
    build.files(files);
    build
        .compile_objects()
        .unwrap_or_else(|e| panic!("nasm failed to assemble libvpx sources: {e}"))
}
