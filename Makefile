SHELL := /bin/bash -e
WORKDIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))

build: 
	docker build -t red-monkey --target executable .

run:
	docker run -it --rm -p 6350:6350 -p 8000:8000 --env-file ./docker.env red-monkey:latest

test:
	docker build -t red-monkey-base --target base . 
	docker run -it --rm red-monkey-base:latest cargo test -- --nocapture

compose-up:
	docker-compose -p red-monkey up red-monkey	

coverage: 
	docker build -t red-monkey-base --target test-coverage .
	# Had to disable ASLR in the docker to run tarpaulin. Check this issue - https://github.com/xd009642/tarpaulin/issues/146
	# Check here for more tarpaulin flags and options - https://github.com/xd009642/tarpaulin#tarpaulin
	docker run --security-opt seccomp=unconfined -it --rm red-monkey-cov:latest cargo tarpaulin -v --bin red-monkey --out Html --exclude-files src/main.rs src/config.rs 

lint:
	docker build -t red-monkey-base --target base .
	docker run -it --rm red-monkey-base:latest cargo clippy -- -D warnings

fmt: 
	docker build -t red-monkey-base --target base .
	docker run -it --rm red-monkey-base:latest cargo fmt 

clean: 
	rm -r ${WORKDIR}/target

doc:
	cargo doc --open --no-deps
