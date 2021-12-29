SHELL := /bin/bash -e
WORKDIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))

build: 
	docker-compose -f ./docker-compose.yml \
		-p proxy build

run:
	docker-compose -f ./docker-compose.yml \
		-p proxy up	

test : 
	cargo test --verbose

test-lint : 
	cargo clippy -- -D warnings

fmt: 
	cargo fmt

clean: 
	rm -r ${WORKDIR}/target
