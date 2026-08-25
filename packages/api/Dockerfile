FROM lukemathwalker/cargo-chef:0.1.78-rust-1.98-slim AS chef
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY src src
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev
ENV SQLX_OFFLINE=true

COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

COPY src src
COPY .sqlx .sqlx
COPY migrations migrations

# Build release binary
RUN cargo build --release --locked --bin hamrah-server

FROM gcr.io/distroless/cc-debian13:nonroot AS runtime

WORKDIR /app

COPY --from=builder --chown=nonroot:nonroot /app/target/release/hamrah-server /app/hamrah-server

USER nonroot:nonroot
ENTRYPOINT ["/app/hamrah-server"]
