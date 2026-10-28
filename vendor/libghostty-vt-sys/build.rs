mod cache;
mod patch;

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Pinned ghostty commit. Update this to pull a newer version.
const GHOSTTY_REPO: &str = "https://github.com/ghostty-org/ghostty.git";
const GHOSTTY_COMMIT: &str = "a887df42c56f6de86c0fe6da9c4eeca37931e083";

#[derive(Clone, Copy)]
enum LinkMode {
    Dynamic,
    Static,
}

impl LinkMode {
    fn current() -> Self {
        if cfg!(feature = "link-dynamic") {
            Self::Dynamic
        } else {
            Self::Static
        }
    }

    fn artifact_kind(self) -> &'static str {
        match self {
            Self::Dynamic => "shared library",
            Self::Static => "static library",
        }
    }

    fn matches_library(self, target: &str, file_name: &str) -> bool {
        match self {
            Self::Dynamic => {
                if target.contains("darwin") {
                    file_name.starts_with("libghostty-vt") && file_name.ends_with(".dylib")
                } else if target.contains("windows") {
                    file_name == "ghostty-vt.lib"
                        || file_name == "ghostty-vt.dll"
                        || file_name == "libghostty-vt.dll.lib"
                        || file_name == "libghostty-vt.dll.a"
                } else {
                    file_name == "libghostty-vt.so" || file_name.starts_with("libghostty-vt.so.")
                }
            }
            Self::Static => {
                if target.contains("windows") {
                    file_name == "ghostty-vt-static.lib"
                } else {
                    file_name == "libghostty-vt.a"
                }
            }
        }
    }

    #[cfg(feature = "pkg-config")]
    fn pkg_config_name(self) -> &'static str {
        match self {
            Self::Dynamic => "libghostty-vt",
            Self::Static => "libghostty-vt-static",
        }
    }
}

fn main() {
    // docs.rs has no Zig toolchain. The checked-in bindings in src/bindings.rs
    // are enough for generating documentation, so skip the entire native
    // build when running under docs.rs.
    if env::var("DOCS_RS").is_ok() {
        return;
    }

    let link_mode = LinkMode::current();
    if env::args_os().nth(1).as_deref() == Some(std::ffi::OsStr::new("--cache-key")) {
        let target = env::var("TARGET").expect("TARGET must be set");
        let host = env::var("HOST").expect("HOST must be set");
        let inputs = cache_inputs(&target, &host, zig_optimize_mode(), link_mode)
            .unwrap_or_else(|error| panic!("native cache fingerprint: {error}"));
        inputs.explain();
        println!("{}", inputs.key());
        return;
    }

    println!("cargo:rerun-if-env-changed=LIBGHOSTTY_VT_SYS_OPTIMIZE");
    println!("cargo:rerun-if-env-changed=GHOSTTY_SOURCE_DIR");
    println!("cargo:rerun-if-env-changed=GHOSTTY_ZIG_SYSTEM_DIR");
    println!("cargo:rerun-if-env-changed=GHOSTTY_BUILD_CACHE_DIR");
    println!("cargo:rerun-if-env-changed=TARGET");
    println!("cargo:rerun-if-env-changed=HOST");
    println!("cargo:rerun-if-env-changed=DEBUG");
    println!("cargo:rerun-if-env-changed=OPT_LEVEL");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=patch.rs");
    println!("cargo:rerun-if-changed=cache.rs");
    for name in patch::PATCHES {
        println!("cargo:rerun-if-changed=patches/{name}");
    }

    // An explicit source override should stay authoritative even when the
    // pkg-config feature is enabled, so local Ghostty checkouts remain easy to
    // test against.
    if env::var_os("GHOSTTY_SOURCE_DIR").is_some() {
        build_vendored(link_mode);
        return;
    }

    // When the pkg-config feature is enabled, prefer an installed library over
    // fetching Ghostty. libghostty is pre-1.0, so this crate intentionally does
    // not promise compatibility with every installed C API revision.
    #[cfg(feature = "pkg-config")]
    if try_pkg_config(link_mode) {
        return;
    }

    build_vendored(link_mode);
}

/// Reuse only completed native installs; Cargo-specific output directories and
/// unrelated Rust features do not affect Ghostty's native build.
fn build_vendored(link_mode: LinkMode) {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set"));
    let target = env::var("TARGET").expect("TARGET must be set");
    let host = env::var("HOST").expect("HOST must be set");
    let optimize = zig_optimize_mode();
    let cache_root = env::var_os("GHOSTTY_BUILD_CACHE_DIR").filter(|value| !value.is_empty());
    let install_prefix = if let Some(root) = cache_root {
        if env::var_os("GHOSTTY_SOURCE_DIR").is_some()
            || env::var_os("GHOSTTY_ZIG_SYSTEM_DIR").is_some()
        {
            println!("cargo:warning=libghostty-vt native cache bypass: source/system override");
            build_install(&out_dir, &target, &host, optimize)
        } else {
            match cache_inputs(&target, &host, optimize, link_mode) {
                Ok(inputs) => cache::install(
                    Path::new(&root),
                    &inputs,
                    |prefix| valid_install(prefix, &target, link_mode),
                    |work| build_install(work, &target, &host, optimize),
                )
                .unwrap_or_else(|error| panic!("libghostty-vt native cache: {error}")),
                Err(error) => {
                    println!("cargo:warning=libghostty-vt native cache bypass: {error}");
                    build_install(&out_dir, &target, &host, optimize)
                }
            }
        }
    } else {
        build_install(&out_dir, &target, &host, optimize)
    };

    assert!(
        valid_install(&install_prefix, &target, link_mode),
        "expected libghostty-vt {} and nonempty ghostty/vt.h in {}",
        link_mode.artifact_kind(),
        install_prefix.display()
    );
    warn_unused_xcframework(&install_prefix.join("lib"));
    for dir in library_search_dirs(&target, &install_prefix) {
        println!("cargo:rustc-link-search=native={}", dir.display());
    }
    match link_mode {
        LinkMode::Dynamic => println!("cargo:rustc-link-lib=dylib=ghostty-vt"),
        LinkMode::Static => println!("cargo:rustc-link-lib=static=ghostty-vt"),
    }
    emit_include_metadata(&[install_prefix.join("include")]);
}

fn valid_install(prefix: &Path, target: &str, link_mode: LinkMode) -> bool {
    let nonempty_file = |path: &Path| {
        std::fs::metadata(path).is_ok_and(|metadata| metadata.is_file() && metadata.len() > 0)
    };
    nonempty_file(&prefix.join("include/ghostty/vt.h"))
        && library_search_dirs(target, prefix).iter().any(|dir| {
            std::fs::read_dir(dir).is_ok_and(|entries| {
                entries.filter_map(Result::ok).any(|entry| {
                    entry.file_name().to_str().is_some_and(|name| {
                        link_mode.matches_library(target, name) && nonempty_file(&entry.path())
                    })
                })
            })
        })
}

/// Build in a private work directory. On a shared-cache miss this directory is
/// inside the unpublished staging tree, so publication is a same-filesystem rename.
fn build_install(out_dir: &Path, target: &str, host: &str, optimize: &str) -> PathBuf {
    let ghostty_dir = match env::var("GHOSTTY_SOURCE_DIR") {
        Ok(dir) => {
            let p = PathBuf::from(dir);
            assert!(
                p.join("build.zig").exists(),
                "GHOSTTY_SOURCE_DIR does not contain build.zig: {}",
                p.display()
            );
            println!("cargo:rerun-if-changed={}", p.display());
            p
        }
        Err(_) => fetch_ghostty(out_dir),
    };
    let ghostty_dir = patch::prepare(&ghostty_dir, out_dir);
    let install_prefix = out_dir.join("ghostty-install");
    let mut build = Command::new("zig");
    build
        .arg("build")
        .arg("-Demit-lib-vt=true")
        .arg(format!("-Doptimize={optimize}"))
        .arg("-Demit-xcframework=false")
        .arg("-Dapp-runtime=none")
        .arg("--prefix")
        .arg(&install_prefix)
        .arg("--cache-dir")
        .arg(out_dir.join("zig-cache"))
        .current_dir(&ghostty_dir)
        .env("GIT_CEILING_DIRECTORIES", out_dir);
    if let Ok(dir) = env::var("GHOSTTY_ZIG_SYSTEM_DIR") {
        assert!(
            !dir.is_empty(),
            "GHOSTTY_ZIG_SYSTEM_DIR must not be empty when set"
        );
        let zig_system_dir = PathBuf::from(dir);
        assert!(
            zig_system_dir.exists(),
            "GHOSTTY_ZIG_SYSTEM_DIR does not exist: {}",
            zig_system_dir.display()
        );
        build
            .arg("--system")
            .arg(&zig_system_dir)
            .arg("--global-cache-dir")
            .arg(out_dir.join("zig-global-cache"));
    }
    if target != host {
        build.arg(format!("-Dtarget={}", zig_target(target)));
    }
    run(build, "zig build");
    install_prefix
}

fn cache_inputs(
    target: &str,
    host: &str,
    optimize: &str,
    link_mode: LinkMode,
) -> std::io::Result<cache::Inputs> {
    use std::io;
    if target != host || !host.ends_with("-apple-darwin") {
        return Err(io::Error::other(
            "shared native installs are limited to native macOS builds",
        ));
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mut inputs = cache::Inputs::default();
    inputs.add("cache-format", b"1");
    inputs.add("repository", GHOSTTY_REPO.as_bytes());
    inputs.add("commit", GHOSTTY_COMMIT.as_bytes());
    inputs.add("target", target.as_bytes());
    inputs.add("host", host.as_bytes());
    inputs.add("optimize", optimize.as_bytes());
    inputs.add("link-mode", link_mode.artifact_kind().as_bytes());
    for name in ["build.rs", "cache.rs", "patch.rs"] {
        inputs.file(&manifest.join(name))?;
    }
    for name in patch::PATCHES {
        inputs.file(&manifest.join("patches").join(name))?;
    }
    let zig = find_executable("zig")?;
    println!("cargo:rerun-if-changed={}", zig.display());
    inputs.add("zig-executable", zig.as_os_str().as_encoded_bytes());
    let wrapper = manifest.join("../../scripts/zig/zig");
    if !is_native_executable(&zig)? {
        if std::fs::canonicalize(&wrapper).ok().as_ref() != Some(&zig) {
            return Err(io::Error::other("unrecognized Zig wrapper"));
        }
        inputs.file(&zig)?;
        let real_zig =
            env::var_os("MUXY_ZIG").ok_or_else(|| io::Error::other("MUXY_ZIG is unset"))?;
        let real_zig = find_executable(&real_zig)?;
        if !is_native_executable(&real_zig)? {
            return Err(io::Error::other("unrecognized MUXY_ZIG wrapper"));
        }
        println!("cargo:rerun-if-changed={}", real_zig.display());
        inputs.add(
            "wrapped-zig-executable",
            real_zig.as_os_str().as_encoded_bytes(),
        );
    }
    for argument in ["version", "targets"] {
        // `targets` includes the native CPU model and feature set. A target
        // triple alone is insufficient when Zig auto-detects the host CPU.
        inputs.add(argument, &command_output("zig", &[argument])?);
    }
    for tool in ["git", "xcrun", "libtool"] {
        let path = find_executable(tool)?;
        if !is_native_executable(&path)? {
            return Err(io::Error::other("unrecognized native build tool wrapper"));
        }
        let tool_path = path
            .to_str()
            .ok_or_else(|| io::Error::other("native build tool path is not UTF-8"))?;
        println!("cargo:rerun-if-changed={}", path.display());
        inputs.add(tool, path.as_os_str().as_encoded_bytes());
        inputs.add(
            "tool-content",
            &command_output("/usr/bin/shasum", &["-a", "256", tool_path])?,
        );
    }
    for argument in ["--show-sdk-path", "--show-sdk-version", "--find"] {
        let args: &[&str] = if argument == "--find" {
            &["--find", "clang"]
        } else {
            &[argument]
        };
        inputs.add(argument, &command_output("xcrun", args)?);
    }
    inputs.add(
        "apple-clang-version",
        &command_output("xcrun", &["clang", "--version"])?,
    );
    // Resolve build tools rather than keying unrelated PATH entries or the
    // isolated smoke HOME. Explicit compiler overrides retain the entire PATH.
    let mut environment: Vec<_> = env::vars_os()
        .filter(|(name, _)| {
            let name = name.to_string_lossy();
            matches!(
                name.as_ref(),
                "SDKROOT"
                    | "DEVELOPER_DIR"
                    | "TOOLCHAINS"
                    | "CC"
                    | "CXX"
                    | "CPP"
                    | "AR"
                    | "LD"
                    | "RANLIB"
                    | "CFLAGS"
                    | "CXXFLAGS"
                    | "CPPFLAGS"
                    | "LDFLAGS"
                    | "CPATH"
                    | "C_INCLUDE_PATH"
                    | "CPLUS_INCLUDE_PATH"
                    | "LIBRARY_PATH"
                    | "MUXY_ZIG"
            ) || name.starts_with("ZIG_")
                || name.ends_with("_DEPLOYMENT_TARGET")
                || ["CC_", "CXX_", "AR_", "CFLAGS_", "CXXFLAGS_", "LDFLAGS_"]
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
        })
        .collect();
    environment.sort();
    if environment.iter().any(|(name, _)| {
        let name = name.to_string_lossy();
        matches!(name.as_ref(), "CC" | "CXX" | "CPP" | "AR" | "LD" | "RANLIB")
            || ["CC_", "CXX_", "AR_"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
    }) {
        inputs.add(
            "compiler-override-PATH",
            env::var_os("PATH").unwrap_or_default().as_encoded_bytes(),
        );
    }
    for name in [
        "PATH",
        "SDKROOT",
        "DEVELOPER_DIR",
        "TOOLCHAINS",
        "CC",
        "CXX",
        "CPP",
        "AR",
        "LD",
        "RANLIB",
        "CFLAGS",
        "CXXFLAGS",
        "CPPFLAGS",
        "LDFLAGS",
        "CPATH",
        "C_INCLUDE_PATH",
        "CPLUS_INCLUDE_PATH",
        "LIBRARY_PATH",
        "MUXY_ZIG",
        "MACOSX_DEPLOYMENT_TARGET",
        "ZIG_LIB_DIR",
        "ZIG_LIBC",
        "ZIG_GLOBAL_CACHE_DIR",
        "ZIG_LOCAL_CACHE_DIR",
        "ZIG_BUILD_RUNNER",
        "IPHONEOS_DEPLOYMENT_TARGET",
        "TVOS_DEPLOYMENT_TARGET",
        "WATCHOS_DEPLOYMENT_TARGET",
        "XROS_DEPLOYMENT_TARGET",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    for (name, value) in environment {
        println!("cargo:rerun-if-env-changed={}", name.to_string_lossy());
        inputs.add(&name.to_string_lossy(), value.as_encoded_bytes());
    }
    Ok(inputs)
}

fn is_native_executable(path: &Path) -> std::io::Result<bool> {
    use std::io::Read;
    let mut magic = [0; 4];
    std::fs::File::open(path)?.read_exact(&mut magic)?;
    Ok(magic == [0x7f, b'E', b'L', b'F']
        || magic[..2] == *b"MZ"
        || matches!(
            magic,
            [0xcf, 0xfa, 0xed, 0xfe]
                | [0xce, 0xfa, 0xed, 0xfe]
                | [0xfe, 0xed, 0xfa, 0xcf]
                | [0xfe, 0xed, 0xfa, 0xce]
                | [0xca, 0xfe, 0xba, 0xbe]
                | [0xbe, 0xba, 0xfe, 0xca]
        ))
}

fn find_executable(name: impl AsRef<std::ffi::OsStr>) -> std::io::Result<PathBuf> {
    let name = Path::new(name.as_ref());
    if name.components().count() > 1 || name.is_absolute() {
        return std::fs::canonicalize(name);
    }
    for directory in env::split_paths(&env::var_os("PATH").unwrap_or_default()) {
        let path = directory.join(name);
        if path.is_file() {
            return std::fs::canonicalize(path);
        }
    }
    Err(std::io::Error::other(format!(
        "cannot locate {}",
        name.display()
    )))
}

fn command_output(program: &str, args: &[&str]) -> std::io::Result<Vec<u8>> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(std::io::Error::other(format!("{program} {args:?} failed")));
    }
    Ok(output.stdout)
}

fn warn_unused_xcframework(lib_dir: &Path) {
    let xcframework = lib_dir.join("ghostty-vt.xcframework");
    if xcframework.exists() {
        println!(
            "cargo:warning=unused libghostty-vt XCFramework emitted at {}; Cargo links the dylib or archive directly",
            xcframework.display()
        );
    }
}

#[cfg(feature = "pkg-config")]
fn try_pkg_config(link_mode: LinkMode) -> bool {
    let mut config = pkg_config::Config::new();
    let lib = match link_mode {
        LinkMode::Dynamic => config.probe(link_mode.pkg_config_name()),
        LinkMode::Static => config
            .statik(true)
            .cargo_metadata(false)
            .probe(link_mode.pkg_config_name()),
    };
    let lib = match lib {
        Ok(lib) => lib,
        Err(_) => return false,
    };

    assert!(
        lib.include_paths.iter().any(|path| {
            std::fs::read_to_string(path.join("ghostty/vt/terminal.h")).is_ok_and(|header| {
                header.contains("ghostty_terminal_clear_screen(")
                    && header.contains("GHOSTTY_TERMINAL_OPT_DEFAULT_GRAPHEME_CLUSTER")
            })
        }),
        "Murvix requires libghostty-vt with patches/clear-screen.patch and patches/grapheme-cluster.patch applied"
    );
    if let LinkMode::Static = link_mode {
        emit_static_pkg_config_metadata(&lib);
    }
    emit_include_metadata(&lib.include_paths);
    true
}

#[cfg(feature = "pkg-config")]
fn emit_static_pkg_config_metadata(lib: &pkg_config::Library) {
    for path in &lib.link_paths {
        println!("cargo:rustc-link-search=native={}", path.display());
    }
    for path in &lib.link_files {
        if let Some(parent) = path.parent() {
            println!("cargo:rustc-link-search=native={}", parent.display());
        }
    }
    for path in &lib.framework_paths {
        println!("cargo:rustc-link-search=framework={}", path.display());
    }
    for framework in &lib.frameworks {
        println!("cargo:rustc-link-lib=framework={framework}");
    }

    println!("cargo:rustc-link-lib=static=ghostty-vt");
    for library in &lib.libs {
        if library != "ghostty-vt" {
            println!("cargo:rustc-link-lib={library}");
        }
    }
    for args in &lib.ld_args {
        if !args.is_empty() {
            println!("cargo:rustc-link-arg=-Wl,{}", args.join(","));
        }
    }
}

fn emit_include_metadata(include_paths: &[PathBuf]) {
    if include_paths.is_empty() {
        return;
    }

    let joined = env::join_paths(include_paths)
        .unwrap_or_else(|error| panic!("failed to join include paths for cargo metadata: {error}"));
    println!("cargo:include={}", joined.to_string_lossy());
}

/// Decide which Zig `OptimizeMode` to pass to `zig build`.
///
/// The `LIBGHOSTTY_VT_SYS_OPTIMIZE` environment variable overrides this unconditionally; accepted
/// values are the four Zig `OptimizeMode` names (`Debug`, `ReleaseSafe`, `ReleaseFast`,
/// `ReleaseSmall`).
///
/// Defaults to `ReleaseFast` for optimized builds. If `DEBUG` is `true` (as cargo sets for the
/// `dev` profile), `Debug` mode is used. Otherwise, if `OPT_LEVEL` is `s` or `z`, `ReleaseSmall`
/// is used.
fn zig_optimize_mode() -> &'static str {
    if let Ok(override_mode) = env::var("LIBGHOSTTY_VT_SYS_OPTIMIZE") {
        return match override_mode.as_str() {
            "Debug" => "Debug",
            "ReleaseSafe" => "ReleaseSafe",
            "ReleaseFast" => "ReleaseFast",
            "ReleaseSmall" => "ReleaseSmall",
            other => panic!(
                "LIBGHOSTTY_VT_SYS_OPTIMIZE must be one of Debug, ReleaseSafe, ReleaseFast, ReleaseSmall (got '{other}')"
            ),
        };
    }

    if env::var("DEBUG").as_deref() == Ok("true") {
        return "Debug";
    }

    match env::var("OPT_LEVEL").as_deref() {
        Ok("s") | Ok("z") => "ReleaseSmall",
        _ => "ReleaseFast",
    }
}

/// Clone ghostty at the pinned commit into OUT_DIR/ghostty-src.
/// Reuses an existing clone if the commit matches.
fn fetch_ghostty(out_dir: &Path) -> PathBuf {
    let src_dir = out_dir.join("ghostty-src");
    let stamp = src_dir.join(".ghostty-commit");

    // Skip fetch if we already have the right commit.
    if stamp.exists()
        && let Ok(existing) = std::fs::read_to_string(&stamp)
        && existing.trim() == GHOSTTY_COMMIT
    {
        return src_dir;
    }

    // Clean and clone fresh.
    if src_dir.exists() {
        std::fs::remove_dir_all(&src_dir)
            .unwrap_or_else(|e| panic!("failed to remove {}: {e}", src_dir.display()));
    }

    eprintln!("Fetching ghostty {GHOSTTY_COMMIT} ...");

    let mut clone = Command::new("git");
    clone
        .arg("clone")
        .arg("--filter=blob:none")
        .arg("--no-checkout")
        .arg(GHOSTTY_REPO)
        .arg(&src_dir);
    run(clone, "git clone ghostty");

    let mut checkout = Command::new("git");
    checkout
        .arg("checkout")
        .arg(GHOSTTY_COMMIT)
        .current_dir(&src_dir);
    run(checkout, "git checkout ghostty commit");

    std::fs::write(&stamp, GHOSTTY_COMMIT).unwrap_or_else(|e| panic!("failed to write stamp: {e}"));

    src_dir
}

fn run(mut command: Command, context: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("failed to execute {context}: {error}"));
    assert!(status.success(), "{context} failed with status {status}");
}

/// Returns directories to search for the built library artifact.
/// On Windows, Zig may place the DLL in `bin/` and the import lib in `lib/`,
/// so both are included.
fn library_search_dirs(target: &str, install_prefix: &Path) -> Vec<PathBuf> {
    let mut dirs = vec![install_prefix.join("lib")];
    if target.contains("windows") {
        dirs.push(install_prefix.join("bin"));
    }
    dirs
}

fn zig_target(target: &str) -> String {
    let value = match target {
        "x86_64-unknown-linux-gnu" => "x86_64-linux-gnu",
        "x86_64-unknown-linux-musl" => "x86_64-linux-musl",
        "aarch64-unknown-linux-gnu" => "aarch64-linux-gnu",
        "aarch64-unknown-linux-musl" => "aarch64-linux-musl",
        "aarch64-apple-darwin" => "aarch64-macos-none",
        "x86_64-apple-darwin" => "x86_64-macos-none",
        "x86_64-pc-windows-gnu" => "x86_64-windows-gnu",
        "aarch64-pc-windows-gnullvm" => "aarch64-windows-gnu",
        "x86_64-pc-windows-msvc" => "x86_64-windows-msvc",
        "aarch64-pc-windows-msvc" => "aarch64-windows-msvc",
        "aarch64-linux-android" => "aarch64-linux-android",
        "x86_64-linux-android" => "x86_64-linux-android",
        other => panic!("unsupported Rust target for vendored build: {other}"),
    };
    value.to_owned()
}
