.PHONY: fmt clippy test test-examples test-release test-examples-release audit

fmt:
	cargo fmt --all --check

clippy:
	cargo clippy --all-targets --all-features -- -D warnings -A clippy::collapsible-if

test:
	cargo test --features des_sim_test_mode

test-examples:
	cargo test --examples --features des_sim_test_mode

test-release:
	cargo test --features des_sim_test_mode --release

test-examples-release:
	cargo test --examples --features des_sim_test_mode --release

audit:
	cargo audit