# Stage 1: Build geoff
FROM rust:1.84-slim AS builder
WORKDIR /src

# Install build dependencies for native crates (openssl, oxigraph/rocksdb, clang-sys)
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev clang cmake make && \
    rm -rf /var/lib/apt/lists/*

COPY . .
RUN cargo build --locked --release -p chapeaux-geoff && \
    strip target/release/geoff

# Stage 2: Runtime
FROM registry.access.redhat.com/ubi9/ubi-minimal:latest

COPY --from=builder /src/target/release/geoff /usr/local/bin/geoff
COPY starters/ /usr/share/geoff/starters/
COPY themes/ /usr/share/geoff/themes/
COPY components/ /usr/share/geoff/components/

# OpenShift compatibility: run as non-root, writable tmp
RUN microdnf install -y git && microdnf clean all
USER 1001
WORKDIR /site
EXPOSE 3000

CMD ["geoff", "serve", "--port", "3000"]
