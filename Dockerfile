# Stage 1: Build frontend
FROM node:20-alpine AS frontend
WORKDIR /app
RUN corepack enable && corepack prepare pnpm@latest --activate
COPY frontend/package.json ./
RUN pnpm install --no-frozen-lockfile
COPY frontend/ .
RUN pnpm build

# Stage 2: Build Rust backend
FROM rust:1-slim AS backend
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && cargo build --release || true
COPY src/ src/
RUN touch src/main.rs && cargo build --release

# Stage 3: Minimal runtime
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=backend /app/target/release/pubsub-tap /usr/local/bin/pubsub-tap
COPY --from=frontend /app/build /app/static

ENV STATIC_DIR=/app/static
ENV PORT=4000
EXPOSE 4000
CMD ["pubsub-tap"]
