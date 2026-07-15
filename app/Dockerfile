# Application Dockerfile for the autoimage editor.
# The Dockerfile at the repo root is the one the editor edits and the build
# workflow consumes; this file is only for running the autoimage web app
# itself. Build with:
#   docker build -f app/Dockerfile -t autoimage .

FROM rust:1-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml ./
COPY src ./src
COPY static ./static
RUN cargo build --release --locked

FROM debian:bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /src/target/release/autoimage /usr/local/bin/autoimage
COPY --from=builder /src/static /app/static
COPY config.toml /app/config.toml
ENV APP_BIND=0.0.0.0:8080
ENV RUST_LOG=info
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/autoimage"]
