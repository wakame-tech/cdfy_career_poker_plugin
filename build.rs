//! Build script: lets the crate link on the host for testing.
//!
//! `lib.rs`'s `#[plugin_fn]` exports reference the extism host ABI (`alloc`,
//! `error_set`, `input_length`, `output_set`, ...). Those imports exist only
//! inside an Extism runtime. On the host, building this crate links its `cdylib`
//! target and the integration-test binaries, both of which would otherwise fail
//! on the unresolved imports. We let the linker leave them for (never-performed)
//! runtime lookup. `rustc-link-arg` covers the cdylib + test binaries but not
//! the rlib, and is emitted only on host OSes — the `wasm32-unknown-unknown`
//! plugin build (`CARGO_CFG_TARGET_OS` = "unknown") is untouched, so the real
//! plugin still gets its extism imports resolved by the runtime.
fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match os.as_str() {
        "macos" | "ios" => {
            println!("cargo:rustc-link-arg=-Wl,-undefined,dynamic_lookup");
        }
        "linux" | "android" => {
            println!("cargo:rustc-link-arg=-Wl,--unresolved-symbols=ignore-all");
        }
        _ => {}
    }
}
