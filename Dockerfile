# syntax=docker/dockerfile:1.23@sha256:2780b5c3bab67f1f76c781860de469442999ed1a0d7992a5efdf2cffc0e3d769

ARG APP_NAME=senec-v3-collector
ARG TARGET=x86_64-unknown-linux-musl

FROM lukemathwalker/cargo-chef:latest-rust-alpine AS chef
ARG TARGET
RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    openssl-libs-static \
    pkgconfig
RUN rustup target add ${TARGET}
RUN cargo install cargo-chef --locked
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
ARG TARGET
ARG APP_NAME
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --target ${TARGET} --recipe-path recipe.json
COPY . .
RUN cargo build --release --target ${TARGET} -p ${APP_NAME}

FROM scratch AS runtime
ARG TARGET
ARG APP_NAME
ENV RUST_LOG=info
WORKDIR /app
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=builder /app/target/${TARGET}/release/${APP_NAME} /usr/local/bin/${APP_NAME}
COPY --from=builder /app/profiles /app/profiles
EXPOSE 9464
VOLUME ["/app/profiles/generated", "/app/state"]
USER 1001:1001
ENTRYPOINT ["/usr/local/bin/senec-v3-collector"]
