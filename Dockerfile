# syntax=docker/dockerfile:1

FROM rust:1.97.1-alpine3.22 AS build

RUN apk add --no-cache musl-dev pax-utils
WORKDIR /src

ENV RUSTFLAGS="-C target-feature=+crt-static"

COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY crates ./crates

RUN cargo build --release --locked --target x86_64-unknown-linux-musl
RUN test -z "$(scanelf --quiet --nobanner --format '%i%n' /src/target/x86_64-unknown-linux-musl/release/alamo2radarr)"

FROM alpine:3.22 AS certificates
RUN apk add --no-cache ca-certificates

FROM scratch

COPY --from=certificates /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=build /src/target/x86_64-unknown-linux-musl/release/alamo2radarr /alamo2radarr

ENV SSL_CERT_FILE=/etc/ssl/certs/ca-certificates.crt

USER 65532:65532
ENTRYPOINT ["/alamo2radarr"]
