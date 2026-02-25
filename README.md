# pubsub-tap

Real-time message viewer for Google Cloud Pub/Sub emulator.

Taps into all topics on your local Pub/Sub emulator and streams messages to a live web dashboard — like browser DevTools for your event bus.

## Features

- Auto-discovers topics on the emulator
- Creates shadow subscriptions to tap messages without affecting your app
- Real-time streaming via Server-Sent Events
- Filter by topic or search message content
- Pause/resume, expandable message detail
- Dark theme dev-tool aesthetic

## Quick Start (Docker Compose)

Add to your `docker-compose.yml`:

```yaml
pubsub-tap:
  image: ghcr.io/kbosompem/pubsub-tap:latest
  ports:
    - "4000:4000"
  environment:
    PUBSUB_EMULATOR_HOST: pubsub-emulator:8085
    GCP_PROJECT_IDS: my-project
  depends_on:
    pubsub-emulator:
      condition: service_healthy
```

Open http://localhost:4000

## Configuration

| Variable | Default | Description |
|---|---|---|
| `PUBSUB_EMULATOR_HOST` | `localhost:8085` | Pub/Sub emulator host:port |
| `GCP_PROJECT_IDS` | `my-project` | Comma-separated project IDs |
| `PORT` | `4000` | HTTP server port |
| `STATIC_DIR` | `./static` | Path to frontend static files |

## Build from Source

```bash
# Frontend
cd frontend && pnpm install && pnpm build && cd ..

# Backend
cargo build --release

# Run
PUBSUB_EMULATOR_HOST=localhost:8085 \
GCP_PROJECT_IDS=my-project \
STATIC_DIR=frontend/build \
./target/release/pubsub-tap
```

## Docker Build

```bash
docker build -t pubsub-tap .
docker run -p 4000:4000 \
  -e PUBSUB_EMULATOR_HOST=host.docker.internal:8085 \
  -e GCP_PROJECT_IDS=my-project \
  pubsub-tap
```

## How It Works

1. The backend polls the Pub/Sub emulator REST API every 500ms
2. Discovers all topics across configured projects
3. Creates `_tap-{topic}` shadow subscriptions on each topic
4. Pulls and acknowledges messages from tap subscriptions
5. Broadcasts messages to all connected browsers via SSE

Shadow subscriptions are independent from your application's subscriptions, so tapping doesn't consume or interfere with your app's messages.

## Architecture

- **Backend**: Rust (axum + tokio) — polls emulator, streams via SSE
- **Frontend**: Svelte 5 — static SPA served by the Rust binary

## License

MIT
