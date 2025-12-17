.PHONY: fmt check test dev-node

fmt:
	cargo fmt

check:
	cargo check

test:
	cargo test

dev-node:
	RUST_LOG=info cargo run --bin cryptochat-node --manifest-path node/Cargo.toml
