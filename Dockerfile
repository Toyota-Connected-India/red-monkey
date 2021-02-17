FROM rust:1.49 AS builder
WORKDIR /build
COPY . /build
RUN cargo build 

FROM debian:buster-slim
COPY --from=builder /build/target/debug/red-monkey /root/red-monkey
## Expose container PORT 
EXPOSE 8000
EXPOSE 6350
CMD ["/root/red-monkey"]