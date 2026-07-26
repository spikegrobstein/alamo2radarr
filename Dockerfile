# syntax=docker/dockerfile:1

FROM rust:1.97.1-alpine3.22 AS build

RUN apk add --no-cache musl-dev pax-utils
WORKDIR /src

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates

RUN cargo build --release --locked
RUN scanelf --nobanner --format '#F%n%i' /src/target/release/alamo2radarr > /tmp/dynamic \
    && test ! -s /tmp/dynamic

FROM alpine:3.22 AS certificates
RUN apk add --no-cache ca-certificates

FROM scratch

COPY --from=certificates /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=build /src/target/release/alamo2radarr /alamo2radarr

ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

USER 65532:65532
ENTRYPOINT ["/alamo2radarr"]
