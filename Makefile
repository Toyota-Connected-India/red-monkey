SHELL := /bin/bash -e
WORKDIR := $(shell dirname $(realpath $(lastword $(MAKEFILE_LIST))))

build: 
	cargo build --release 

run:
	cargo run

docker-build: 
	docker build -t red-monkey .

docker-run:
	docker run -it --rm -p 6350:6350 -p 8000:8000 --env-file ./docker.env red-monkey:latest

docker-componse-build: 
	docker-compose -f ./docker-compose.yml \
		-p proxy build

docker-compose-run:
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
