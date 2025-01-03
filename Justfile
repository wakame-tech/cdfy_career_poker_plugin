default:
    cargo build --target wasm32-wasi
    cp ./target/wasm32-wasi/debug/cdfy_plugin_career_poker.wasm ../cdfy-node/
