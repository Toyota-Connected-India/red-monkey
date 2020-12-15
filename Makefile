SHELL := /bin/bash -e
WORKDIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))

.PHONY: test-unit
test-unit : 
	cargo test --verbose

.PHONY: test-lint
test-lint : 
	cargo clippy -- -D warnings

.PHONY: build
build : 
	cargo build --verbose

.PHONY: test
test: test-unit test-lint

.PHONY: fmt
fmt: 
	cargo fmt

.PHONY: clean
clean: 
	rm -r ${WORKDIR}/target
