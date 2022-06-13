# ------------------ Base image ------------------
FROM rust:1.59 as base 

WORKDIR /usr/src/app

COPY Cargo.toml .
COPY ./src src
RUN mkdir .cargo
RUN cargo vendor > .cargo/config
RUN cat .cargo/config
RUN rustup component add rustfmt clippy;

# ------------------- Builder -------------------- 

FROM base AS builder
RUN cargo build --release 
RUN cargo install --path . --verbose

# ---------------- Executable image --------------

FROM debian:buster-slim as executable 
COPY --from=builder /usr/local/cargo/bin/red-monkey /bin

RUN apt-get update \
 && apt-get install -y ca-certificates

RUN apt install libssl1.1

EXPOSE 8000
EXPOSE 6350

CMD ["/bin/red-monkey"]

# ----------------- Test Coverage -----------------

FROM rust:1.59 as test-coverage 

WORKDIR /usr/src/app

COPY Cargo.lock .
COPY Cargo.toml .
COPY ./src src

RUN cargo install cargo-tarpaulin
