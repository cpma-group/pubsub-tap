# Stage 1: Build frontend
FROM node:22-alpine AS frontend
WORKDIR /app
RUN corepack enable
COPY frontend/package.json frontend/pnpm-lock.yaml ./
RUN pnpm install --frozen-lockfile
COPY frontend/ .
RUN pnpm build

# Stage 2: Build Rust backend
FROM rust:1-slim AS backend
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
RUN cargo build --release --locked

# Stage 3: Minimal runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend /app/target/release/pubsub-tap /usr/local/bin/pubsub-tap
COPY --from=frontend /app/build /app/static

ENV STATIC_DIR=/app/static
ENV PORT=4000
EXPOSE 4000
HEALTHCHECK --interval=10s --timeout=3s --retries=3 --start-period=3s \
  CMD curl -f http://127.0.0.1:4000/healthz || exit 1
CMD ["pubsub-tap"]
