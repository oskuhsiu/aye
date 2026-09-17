.PHONY: install check test integration
PYTHON ?= python3
install:
	cargo install --path . --force --locked
check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test
test: check
integration: check install
	$(PYTHON) verification/installed.py
