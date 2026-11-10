use std::{env, path::PathBuf};

fn main() {
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let pocket = root.join("../..");
    println!("cargo:rerun-if-env-changed=POCKET_QUICKJS_DIR");
    println!("cargo:rerun-if-env-changed=POCKETJS_QJS_OPT_LEVEL");
    let quickjs_source = PathBuf::from(
        env::var_os("POCKET_QUICKJS_DIR")
            .expect("set POCKET_QUICKJS_DIR to the QuickJS-NG 0.16.2 source directory"),
    );
    let bare_metal = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none");
    println!("cargo:rerun-if-changed={}", quickjs_source.display());
    let quickjs = if bare_metal {
        // QuickJS 0.16.2 declares pthread helpers even when JS atomics are off.
        // Build a private copy with those unused helpers excluded on bare metal.
        let destination = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("quickjs");
        std::fs::create_dir_all(&destination).unwrap();
        for entry in std::fs::read_dir(&quickjs_source).unwrap() {
            let entry = entry.unwrap();
            if matches!(
                entry.path().extension().and_then(|s| s.to_str()),
                Some("c" | "h")
            ) {
                std::fs::copy(entry.path(), destination.join(entry.file_name())).unwrap();
            }
        }
        let header = destination.join("cutils.h");
        let source = std::fs::read_to_string(&header).unwrap();
        let include = "#if !defined(_WIN32) && !defined(EMSCRIPTEN) && !defined(__wasi__) && !defined(__DJGPP)";
        let threads = "#if defined(EMSCRIPTEN) || defined(__wasi__) || defined(__DJGPP)";
        assert!(
            source.contains(include) && source.contains(threads),
            "unsupported QuickJS cutils.h; use the documented 0.16.2 source"
        );
        std::fs::write(
            header,
            source
                .replace(
                    include,
                    &format!("{include} && !defined(POCKET_EMBASSY_BARE_METAL)"),
                )
                .replace(
                    threads,
                    &format!("{threads} || defined(POCKET_EMBASSY_BARE_METAL)"),
                ),
        )
        .unwrap();
        destination
    } else {
        quickjs_source
    };
    let mut build = cc::Build::new();
    build
        .include(&quickjs)
        .include(pocket.join("contracts/generated"))
        .include(pocket.join("engine/guest-quickjs/include"))
        .include(pocket.join("engine/guest-quickjs/src"))
        .flag_if_supported("-std=gnu11")
        .flag_if_supported("-fno-strict-aliasing")
        .define("NDEBUG", None)
        .define("__STDC_NO_ATOMICS__", "1")
        .define("malloc", "pocket_c_malloc")
        .define("calloc", "pocket_c_calloc")
        .define("realloc", "pocket_c_realloc")
        .define("free", "pocket_c_free")
        .define("malloc_size", "pocket_c_usable_size")
        .define("malloc_usable_size", "pocket_c_usable_size")
        .warnings(false);
    if bare_metal {
        build
            .define("POCKET_EMBASSY_BARE_METAL", None)
            .define("clock_gettime", "pocket_embassy_clock_gettime")
            .define("gettimeofday", "pocket_embassy_gettimeofday")
            .flag("-include")
            .flag(root.join("native/baremetal.h").to_str().unwrap());
    }
    if let Ok(level) = env::var("POCKETJS_QJS_OPT_LEVEL") {
        build.opt_level_str(&level);
    }
    for file in ["quickjs.c", "dtoa.c", "libregexp.c", "libunicode.c"] {
        let path = quickjs.join(file);
        build.file(path);
    }
    for path in [
        pocket.join("engine/guest-quickjs/src/guest.c"),
        root.join("native/guest.c"),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
        build.file(path);
    }
    println!("cargo:rerun-if-changed=native/baremetal.h");
    println!(
        "cargo:rerun-if-changed={}",
        pocket.join("engine/guest-quickjs/include").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        pocket.join("contracts/generated").display()
    );
    build.compile("pocket_embassy_quickjs");
}
