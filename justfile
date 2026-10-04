set windows-shell := ["powershell.exe", "-Command"]
set export := true

# --- Lints:

check: fmt clippy

fmt:
    cargo fmt --all

clippy:
    cargo clippy --no-deps --all-features --tests --benches -- \
        -D clippy::all \
        -D clippy::pedantic \
        -D clippy::nursery

# --- Misc:

clean:
    cargo clean

test:
    cargo test