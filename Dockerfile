FROM rust:1.100-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY contracts ./contracts

# Build gate: Render must compile and pass the Rust unit tests before deploying
# the existing worker replacement. Test failures leave the old deploy running.
RUN cargo test --locked --all-targets

RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/sandooo /usr/local/bin/sandooo
RUN mkdir -p /app/cache

CMD ["sandooo"]
