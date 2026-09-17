.PHONY: install test check benchmark

install:
	cargo install --path . --force --locked

check:
	cargo fmt --check
	cargo clippy --all-targets -- -D warnings
	cargo test

test: check install
	python3 verification/check.py Bootstrap
	python3 -m unittest discover -s verification -p '*_cases.py'

benchmark: install
	python3 verification/benchmark.py
