# Build: compile the Rust binary (SQLite is compiled in, so the runtime image needs no extra libraries).
FROM docker.io/library/rust:1-slim-trixie AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
COPY words ./words
RUN cargo build --release --locked

# Run: Debian with the binary, the front end and the Claude Code CLI (used only by the generator service).
FROM docker.io/library/debian:trixie-slim
ARG CLAUDE_CODE_VERSION=2.1.314
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl \
 && HOME=/opt/claude-install bash -c "curl -fsSL https://claude.ai/install.sh | bash -s ${CLAUDE_CODE_VERSION}" \
 && cp -L /opt/claude-install/.local/bin/claude /usr/local/bin/claude \
 && rm -rf /opt/claude-install \
 && apt-get purge -y curl && apt-get autoremove -y && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/quikflip /app/quikflip
COPY web /app/web
# HOME=/tmp gives the CLI a writable home for its own config; DISABLE_AUTOUPDATER keeps the pinned version.
ENV QUIKFLIP_DATA_DIR=/data \
    QUIKFLIP_WEB_DIR=/app/web \
    QUIKFLIP_BIND=0.0.0.0:8080 \
    QUIKFLIP_TIMEZONE=Europe/Brussels \
    QUIKFLIP_CLAUDE_BIN=/usr/local/bin/claude \
    DISABLE_AUTOUPDATER=1 \
    HOME=/tmp
EXPOSE 8080
VOLUME /data
# 568 is the "apps" user TrueNAS SCALE uses for app datasets.
USER 568:568
ENTRYPOINT ["/app/quikflip"]
CMD ["serve"]
