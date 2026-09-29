dev:
    python3 -m http.server 3000 & \
    watchexec -w src -w Cargo.toml -w Cargo.lock -- \
    wasm-pack build --target web
