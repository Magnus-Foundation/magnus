ARG CHEF_IMAGE=chef

FROM ${CHEF_IMAGE} AS builder

ARG TARGETARCH
ARG RUST_PROFILE=profiling
ARG RUST_FEATURES="asm-keccak,jemalloc,otlp"
ARG VERGEN_GIT_SHA
ARG VERGEN_GIT_SHA_SHORT
ARG EXTRA_RUSTFLAGS=""

COPY . .

# Build ALL binaries in one pass - they share compiled artifacts
RUN --mount=type=cache,target=/usr/local/cargo/registry,sharing=locked,id=cargo-registry-${TARGETARCH} \
    --mount=type=cache,target=/usr/local/cargo/git,sharing=locked,id=cargo-git-${TARGETARCH} \
    --mount=type=cache,target=$SCCACHE_DIR,sharing=locked,id=sccache-${TARGETARCH} \
    RUSTFLAGS="-C link-arg=-fuse-ld=mold ${EXTRA_RUSTFLAGS}" \
    cargo build --profile ${RUST_PROFILE} \
        --bin magnus --features "${RUST_FEATURES}" \
        --bin magnus-bench \
        --bin magnus-sidecar \
        --bin magnus-xtask

FROM debian:bookworm-slim AS base

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /data

# magnus
FROM base AS magnus
ARG RUST_PROFILE=profiling
COPY --from=builder /app/target/${RUST_PROFILE}/magnus /usr/local/bin/magnus
ENTRYPOINT ["/usr/local/bin/magnus"]

# magnus-sidecar
FROM base AS magnus-sidecar
ARG RUST_PROFILE=profiling
COPY --from=builder /app/target/${RUST_PROFILE}/magnus-sidecar /usr/local/bin/magnus-sidecar
ENTRYPOINT ["/usr/local/bin/magnus-sidecar"]

# magnus-xtask
FROM base AS magnus-xtask
ARG RUST_PROFILE=profiling
COPY --from=builder /app/target/${RUST_PROFILE}/magnus-xtask /usr/local/bin/magnus-xtask
ENTRYPOINT ["/usr/local/bin/magnus-xtask"]

# magnus-bench (needs nushell)
FROM --platform=$TARGETPLATFORM ghcr.io/nushell/nushell:0.108.0-bookworm AS nushell

FROM base AS magnus-bench
ARG RUST_PROFILE=profiling
COPY --from=nushell /usr/bin/nu /usr/bin/nu
COPY --from=builder /app/target/${RUST_PROFILE}/magnus-bench /usr/local/bin/magnus-bench
ENTRYPOINT ["/usr/local/bin/magnus-bench"]
