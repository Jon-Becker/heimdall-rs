.PHONY: all build build-release check clean fmt format lint lint-fix test test-doc test-heavy test-cov test-cov-json bench install help eval

# Clippy flags used across the project
CLIPPY_ALLOW := --allow clippy::new_without_default \
	--allow clippy::redundant_field_names \
	--allow clippy::too_many_arguments \
	--allow clippy::format_in_format_args \
	--allow clippy::should_implement_trait

ci: fmt lint test

help:
	@echo "Available targets:"
	@echo "  make build         - Build the project (debug)"
	@echo "  make build-release - Build the project (release)"
	@echo "  make check         - Run cargo check"
	@echo "  make clean         - Clean build artifacts"
	@echo "  make fmt           - Format code with rustfmt (nightly)"
	@echo "  make format        - Alias for fmt"
	@echo "  make lint          - Run clippy"
	@echo "  make lint-fix      - Run clippy with auto-fix"
	@echo "  make test          - Run tests with nextest"
	@echo "  make test-doc      - Run doc tests"
	@echo "  make test-heavy    - Run ignored/heavy integration tests"
	@echo "  make test-cov      - Run tests with coverage (cobertura XML)"
	@echo "  make test-cov-json - Run tests with coverage (JSON)"
	@echo "  make bench         - Run benchmarks"
	@echo "  make install       - Install heimdall CLI"

build:
	cargo build --workspace

build-release:
	cargo build --workspace --release

check:
	cargo check --workspace --all-targets --all-features

clean:
	cargo clean

fmt format:
	cargo +nightly fmt --all

fmt-check:
	cargo +nightly fmt --check --all

lint:
	cargo clippy --all-features -- $(CLIPPY_ALLOW)

lint-fix:
	cargo +nightly clippy --fix --all-features -Z unstable-options --allow-dirty --allow-staged -- $(CLIPPY_ALLOW)

test:
	cargo nextest r --no-fail-fast --release

test-doc:
	cargo test --workspace --doc

test-heavy:
	cargo nextest r --no-fail-fast --release --run-ignored all

# Ignore pattern for coverage
COV_IGNORE := --ignore-filename-regex='.*dump.*\.rs|.*rpc\.rs|.*logging\.rs|.*http\.rs|.*transpose\.rs|.*resources.*\.rs|.*test(s)?\.rs|main\.rs|.*lib\.rs'

test-cov:
	cargo llvm-cov --release $(COV_IGNORE) --cobertura --output-path coverage.xml

test-cov-json:
	cargo llvm-cov --release $(COV_IGNORE) --json --output-path coverage.json

bench:
	cargo bench -p heimdall-core

install:
	cargo install --path crates/cli --locked

eval:
	# note: needs heimdall-eval cloned locally next to heimdall-rs
	@ cd ../heimdall-eval && make eval-all DEV=1 > /dev/null 2>&1
	@ cat ../heimdall-eval/heimdall/evals.json
