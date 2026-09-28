# Run the CI checks that work locally. Worth doing before pushing.
check: fmt clippy test docs

# Check formatting. Needs nightly, since rustfmt.toml uses unstable options.
fmt:
    cargo +nightly fmt --all --check

# Run Clippy for this platform, and for each other CI platform whose target is installed.
[script("bash")]
clippy:
    set -euo pipefail
    clippy() {
        cargo clippy -p opener --all-targets "$@" -- -D warnings
        cargo clippy --workspace --all-targets --all-features "$@" -- -D warnings
    }
    clippy
    host=$(rustc -vV | sed -n 's/^host: //p')
    installed=$(rustup target list --installed)
    for target in x86_64-unknown-linux-gnu x86_64-pc-windows-msvc aarch64-apple-darwin \
        x86_64-unknown-freebsd i686-pc-windows-msvc; do
        if [[ $target == "$host" ]]; then
            continue
        elif grep -qx "$target" <<< "$installed"; then
            clippy --target "$target"
        else
            echo "Skipping Clippy for $target. To check it, run: rustup target add $target" >&2
        fi
    done

test:
    cargo test --workspace --all-features

# Build the docs as docs.rs does. `--cfg docsrs` goes only to this crate, as on docs.rs.
docs:
    cargo +nightly rustdoc -p opener --all-features -- --cfg docsrs -D warnings
