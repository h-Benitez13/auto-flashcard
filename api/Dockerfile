# Build stage
FROM rust:1-bookworm AS builder

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install poppler-utils for pdftotext (best PDF extraction quality).
# If poppler is unavailable at runtime, the API falls back to pure-Rust extractors.
RUN apt-get update \
    && apt-get install -y --no-install-recommends poppler-utils ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/flashcards-api ./flashcards-api

EXPOSE 3001

# DATABASE_PATH defaults to /data/app.db; fly.toml mounts a persistent volume at /data.
ENV DATABASE_PATH=/data/app.db
ENV PORT=3001

CMD ["./flashcards-api"]
