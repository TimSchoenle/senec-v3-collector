# syntax=docker/dockerfile:1.28@sha256:bb22d9815c728170f72750f4e5b0d672e06176142e1d602c7e66c050100b7e5b

ARG APP_NAME=senec-v3-collector
ARG TARGET=x86_64-unknown-linux-musl

# Pinned by digest, which Renovate bumps like the other base images. The floating tag alone let two
# builds of one commit compile with different toolchain images and cargo-chef releases.
FROM lukemathwalker/cargo-chef:latest-rust-alpine@sha256:c48fa69aff194617af79ddc9473a58b946068012721979a9aef9c3e1466ccbd8 AS chef
ARG TARGET
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig
WORKDIR /app

# The compiler `rust-toolchain.toml` pins, installed before any stage forks. Without it the
# toolchain would be whichever release the base image happens to carry, and only the stages that
# `COPY . .` would see the pin: `cargo chef cook` and `cargo build` would then compile with
# different rustc releases, and the build would recompile every dependency the cook just cached.
# Copied on its own so that no other change to the tree invalidates this layer.
COPY rust-toolchain.toml ./
RUN rustup toolchain install --no-self-update
RUN rustup target add ${TARGET}
# The cargo-chef release the base image above ships, installed again for the toolchain
# `rust-toolchain.toml` pins (a no-op while the two agree). Exact, so a bot, not a crates.io release,
# is what moves it; Renovate's `cargo install` manager in renovate.json bumps it.
RUN cargo install cargo-chef --locked --version 0.1.78

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG TARGET
ARG APP_NAME
COPY --from=planner /app/recipe.json recipe.json
# `--locked` on both steps: a Cargo.lock that no longer matches the manifests fails the build
# instead of being re-resolved inside the image, so the binary is built from the versions the
# repository records and the dependency review checked.
RUN cargo chef cook --release --locked --target ${TARGET} --recipe-path recipe.json
COPY . .
RUN cargo build --release --locked --target ${TARGET} -p ${APP_NAME}

FROM scratch AS runtime
ARG TARGET
ARG APP_NAME
ENV RUST_LOG=info
WORKDIR /app
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /app/target/${TARGET}/release/${APP_NAME} /usr/local/bin/${APP_NAME}
EXPOSE 9464
VOLUME ["/app/profiles/generated", "/app/state"]
USER 1001:1001
ENTRYPOINT ["/usr/local/bin/senec-v3-collector"]
