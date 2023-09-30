FROM rustlang/rust:nightly-bookworm AS builder
RUN update-ca-certificates

RUN useradd rust --user-group --create-home --shell /bin/bash --groups sudo
USER rust
RUN mkdir -p /home/rust/libs /home/rust/src /home/rust/.cargo && \
    ln -s /opt/rust/cargo/config /home/rust/.cargo/config

WORKDIR /home/rust/src

ADD --chown=rust:rust . ./
RUN USER=rust cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libc6 libssl3 libpq5 ca-certificates p11-kit-modules gnutls-bin \
    libengine-pkcs11-openssl && apt-get clean && rm -rf /var/lib/apt/lists/*
RUN update-ca-certificates

COPY --from=builder --chown=0:0 /etc/ssl/certs /etc/ssl/certs
COPY --from=builder --chown=0:0 /home/rust/src/target/release/shadow-weaver-controller /shadow-weaver-controller
COPY --from=builder --chown=0:0 /home/rust/src/target/release/shadow-weaver-inject /shadow-weaver-inject

ENTRYPOINT ["/shadow-weaver-controller"]