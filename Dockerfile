# --- build stage -----------------------------------------------------------
FROM rust:1-slim AS builder
WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY migrations ./migrations

# Mount cargo registry and git cache, and compile using cache mounts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    cp /app/target/release/hakiki-api /usr/local/bin/hakiki-api

# --- runtime stage ----------------------------------------------------------
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /usr/local/bin/hakiki-api /usr/local/bin/hakiki-api
ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
CMD ["hakiki-api"]
