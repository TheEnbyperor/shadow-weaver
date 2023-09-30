FROM rustlang/rust:nightly-bookworm AS builder
RUN update-ca-certificates
WORKDIR /usr/src/

RUN USER=root cargo new shadow-weaver-spider
WORKDIR /usr/src/shadow-weaver-spider
COPY Cargo.toml Cargo.lock ./
RUN cargo build --release

COPY src ./src
RUN cargo install --path .

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libc6 libssl3 libpq5 ca-certificates p11-kit-modules gnutls-bin \
    libengine-pkcs11-openssl && apt-get clean && rm -rf /var/lib/apt/lists/*
RUN update-ca-certificates

COPY --from=builder --chown=0:0 /etc/ssl/certs /etc/ssl/certs
COPY --from=builder --chown=0:0 /usr/local/cargo/bin/shadow-weaver-spider /shadow-weaver-spider

ENTRYPOINT ["/shadow-weaver-spider"]