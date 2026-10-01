[script]
dev:
    python3 -m http.server 3000 &
    PYTHON_PID=$!
    trap 'kill $PYTHON_PID 2>/dev/null; wait $PYTHON_PID 2>/dev/null' EXIT INT TERM

    watchexec -w src -w Cargo.toml -w Cargo.lock -- \
        wasm-pack build --target web
