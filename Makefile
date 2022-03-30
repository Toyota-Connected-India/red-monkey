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
