# Fail on early and on unset variables in non-shebang recipes
set shell := ["bash", "-euo", "pipefail", "-c"]
# Allow usage of bash methods to handle multiple arguments and work around quoting issues
set positional-arguments
set quiet

@default: fmt lint test

test:
	cargo test --workspace --all-targets --all-features
	# cargo test --workspace --doc --all-features

lint:
    cargo '+nightly' fmt -- --check
    cargo clippy \
        --workspace \
        --tests \
        --benches \
        --all-targets \
        --all-features \
        --quiet

    cargo doc --all --no-deps --document-private-items --all-features --quiet

fmt:
	cargo '+nightly' fmt

udeps:
	cargo '+nightly' udeps

install-nightly:
	rustup toolchain install nightly
