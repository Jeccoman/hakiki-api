# --- build stage -----------------------------------------------------------
# Uses the image's own toolchain (the .cargo/config.toml rustc pin is excluded
# via .dockerignore — that hack is only for the host's old /usr/bin/rustc).
FROM rust:1-slim AS builder
WORKDIR /app

# Cache dependencies separately from source.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src

COPY src ./src
COPY migrations ./migrations
RUN touch src/main.rs && cargo build --release

# --- runtime stage ----------------------------------------------------------
# distroless: no shell, minimal attack surface. rustls means no OpenSSL needed.
FROM gcr.io/distroless/cc-debian12
COPY --from=builder /app/target/release/hakiki-api /usr/local/bin/hakiki-api
ENV BIND_ADDR=0.0.0.0:8080
EXPOSE 8080
CMD ["hakiki-api"]
