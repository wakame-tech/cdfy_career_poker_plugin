//! Host RNG bridge.
//!
//! The only source of nondeterminism the plugin uses. cdfy_next's core draws
//! from a room-seeded SplitMix64 (`core::rng::RngState`) and exposes it as the
//! `rand_u64` host function in the `extism:host/user` namespace.
//!
//! In the Rust extism-pdk a scalar `-> u64` host fn auto-reads the 8
//! little-endian bytes the host wrote into Extism memory and returns the
//! decoded value (confirmed against cdfy_next `test-plugin`, which calls
//! `rand_u64()?` directly). So no manual memory read is needed on the Rust side
//! — unlike the MoonBit plugin, which reads the bytes by hand.

#[cfg(target_arch = "wasm32")]
use extism_pdk::host_fn;

#[cfg(target_arch = "wasm32")]
#[host_fn]
extern "ExtismHost" {
    fn rand_u64() -> u64;
}

/// Next draw from the host RNG. On wasm this calls the host import; on the host
/// build (unit tests) it returns 0 — tests inject their own closures into the
/// rules/deck and never rely on this path.
pub fn next_u64() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        unsafe { rand_u64().expect("rand_u64 host fn failed") }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

/// A mutable `FnMut() -> u64` source backed by the host RNG, for passing into
/// `Deck::shuffle_with` / rules functions.
pub fn host_source() -> impl FnMut() -> u64 {
    next_u64
}
