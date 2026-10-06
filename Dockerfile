# syntax=docker/dockerfile:1
# The tada image (ADRs 0025, 0028 and 0041): the binary and the built web client on distroless.
# Renovate updates the base images and keeps them pinned by digest.

FROM lukemathwalker/cargo-chef:0.1.78-rust-1.99-slim-trixie@sha256:bcda2cef02bedabfaf433379d82fecd02c0a428fb66fe28ab2d65c25c3db3b5e AS chef
WORKDIR /build
# The base image has the toolchain of rust-toolchain.toml; this name stops rustup from a second download.
ENV RUSTUP_TOOLCHAIN=1.99.0

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# A change of the source does not rebuild the dependencies: they are in their own layer.
FROM chef AS rust
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --locked --package tada --recipe-path recipe.json
COPY . .
ENV SQLX_OFFLINE=true
RUN cargo build --release --locked --package tada

FROM node:24.21.0-trixie-slim@sha256:173f125896c3b47ddf056734c7ea789d04595a6a08769a8f78e0df642781fb66 AS web
RUN npm install --global pnpm@12.9.1
WORKDIR /build/apps/web
COPY apps/web/package.json apps/web/pnpm-lock.yaml apps/web/pnpm-workspace.yaml ./
RUN pnpm install --frozen-lockfile
COPY contracts /build/contracts
COPY locales /build/locales
COPY apps/web ./
RUN pnpm generate && pnpm build

FROM gcr.io/distroless/cc-debian13:nonroot@sha256:e792ab3d241a468a4fd7519ddbbebe66b49b5f365771716ea688ad40b6c6f1c2
COPY --from=rust /build/target/release/tada /usr/local/bin/tada
COPY --from=web /build/apps/web/dist /app/web
ENV TADA_WEB_ROOT=/app/web
USER nonroot
EXPOSE 8080
ENTRYPOINT ["/usr/local/bin/tada"]
CMD ["serve"]
