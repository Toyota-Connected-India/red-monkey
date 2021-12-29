### Builder image
FROM rust:1.57 AS builder
WORKDIR /usr/src/app

COPY Cargo.lock .
COPY Cargo.toml .
RUN mkdir .cargo
RUN cargo vendor > .cargo/config
RUN cat .cargo/config
COPY ./src src

RUN rustup default nightly-2021-12-10
RUN cargo build --release 
RUN cargo install --path . --verbose

### Final light-weight image 
FROM debian:buster-slim
COPY --from=builder /usr/local/cargo/bin/red-monkey /bin

EXPOSE 8000
EXPOSE 6350

CMD ["/bin/red-monkey"]
