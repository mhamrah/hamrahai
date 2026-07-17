FROM lukemathwalker/cargo-chef:0.1.77-rust-1.97-slim AS chef
WORKDIR /app

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
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

# Create non-root user
RUN useradd -m -u 1000 appuser
USER appuser

CMD ["/app/target/release/hamrah-server"]
