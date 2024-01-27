```bash
cargo build --release --target wasm32-wasi
cp ./target/wasm32-wasi/release/cdfy_career_poker_plugin.wasm ../cdfy/cdfy_room_server/plugins/
```