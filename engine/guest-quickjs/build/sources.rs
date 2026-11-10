use std::path::Path;

/// Add the C engine and canonical guest to a program-owned cc build.
/// The caller selects the compiler, optimization, libc, clocks and link settings.
pub fn add(build: &mut cc::Build, pocket: &Path, quickjs: &Path, bare_metal: bool) {
    build
        .include(quickjs)
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
        .define("malloc_usable_size", "pocket_c_usable_size");
    if bare_metal {
        build
            .define("POCKET_QJS_NO_THREADS", None)
            .define("clock_gettime", "pocket_qjs_clock_gettime")
            .define("gettimeofday", "pocket_qjs_gettimeofday")
            .flag("-include")
            .flag(
                pocket
                    .join("engine/guest-quickjs/include/pocket_quickjs_clock.h")
                    .to_str()
                    .unwrap(),
            );
    }
    for file in ["quickjs.c", "dtoa.c", "libregexp.c", "libunicode.c"] {
        let path = quickjs.join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        build.file(path);
    }
    for entry in std::fs::read_dir(quickjs).expect("QuickJS source directory") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "h") {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
    for file in ["src/guest.c", "src/realm.c"] {
        let path = pocket.join("engine/guest-quickjs").join(file);
        println!("cargo:rerun-if-changed={}", path.display());
        build.file(path);
    }
    for directory in ["engine/guest-quickjs/include", "contracts/generated"] {
        println!(
            "cargo:rerun-if-changed={}",
            pocket.join(directory).display()
        );
    }
    println!(
        "cargo:rerun-if-changed={}",
        pocket
            .join("engine/guest-quickjs/src/generated_ops.h")
            .display()
    );
}
