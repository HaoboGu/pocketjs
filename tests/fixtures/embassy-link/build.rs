#[path = "../../../engine/guest-quickjs/build/sources.rs"]
mod sources;
use std::{env, path::PathBuf};
fn main() {
    println!("cargo:rerun-if-env-changed=POCKET_QUICKJS_DIR");
    let root = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let pocket = root.join("../../..");
    let quickjs = PathBuf::from(
        env::var_os("POCKET_QUICKJS_DIR").expect("set POCKET_QUICKJS_DIR to the prepared source"),
    );
    let bare = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("none");
    let mut build = cc::Build::new();
    sources::add(&mut build, &pocket, &quickjs, bare);
    if bare {
        build.file(pocket.join("tests/fixtures/embassy-link/clock.c"));
    }
    build.warnings(false).compile("fixture_quickjs");
}
