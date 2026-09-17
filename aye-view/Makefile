.PHONY: install check test
install:
	cargo install --path . --force --locked
check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test
test: check
