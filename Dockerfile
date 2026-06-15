FROM rust:1-alpine3.24
ENV RUSTFLAGS="-C strip=debuginfo"
WORKDIR /app
COPY . .
RUN --mount=type=cache,target=/var/cache/buildkit \
    CARGO_HOME=/var/cache/buildkit/cargo \
    CARGO_TARGET_DIR=/var/cache/buildkit/target \
    cargo build --release --locked && \
    cp -v /var/cache/buildkit/target/release/patchup /

FROM alpine:3.24
COPY contrib/hub.toml /etc/patchup/hub.toml
COPY --from=0 /patchup /
USER nobody
VOLUME ["/data"]
ENV PATCHUP_HUB_DATA=/data
ENTRYPOINT ["/patchup"]
