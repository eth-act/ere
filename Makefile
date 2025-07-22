# Heavily inspired by Reth: https://github.com/paradigmxyz/reth/blob/4c39b98b621c53524c6533a9c7b52fc42c25abd6/Makefile
.DEFAULT_GOAL := help

##@ Help
.PHONY: help
help: # Display this help.
	@awk 'BEGIN {FS = ":.*#"; printf "Usage:\n  make \033[34m<target>\033[0m\n"} /^[a-zA-Z_0-9-]+:.*?#/ { printf "  \033[34m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) }' $(MAKEFILE_LIST)

##@ Build
.PHONY: build
build: # Build the Ream binary into `target` directory.
	@cargo build --verbose --release


##@ Lint
.PHONY: clean
clean: # Run `cargo clean`.
	@cargo clean

.PHONY: lint pr
lint: # Run `clippy` and `rustfmt`.
	cargo +nightly fmt --all
	cargo clippy --all --all-targets --no-deps -- --deny warnings

	# clippy for bls with supranational feature
	cargo clippy --all-targets --no-deps -- --deny warnings

	# cargo sort
	cargo sort --grouped
