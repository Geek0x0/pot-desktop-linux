FROM ubuntu:24.04 AS base

ENV DEBIAN_FRONTEND=noninteractive
ENV CARGO_HOME=/usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH="${CARGO_HOME}/bin:${PATH}"

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libgtk-4-dev \
    libadwaita-1-dev \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    libx11-dev \
    libssl-dev \
    gettext \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y \
    --profile minimal \
    --default-toolchain 1.88 \
    && rustup component add clippy \
    && cargo install cargo-deb cargo-generate-rpm \
    && rm -rf "${CARGO_HOME}/registry/src" "${CARGO_HOME}/registry/cache"

WORKDIR /src

# ─── Dependency cache layer ──────────────────────────────────────────────────
FROM base AS deps
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs \
    && cargo build --release \
    && rm -rf src

# ─── Builder ─────────────────────────────────────────────────────────────────
FROM deps AS builder
COPY . .
ARG FEATURES=""
ARG CARGO_FLAGS=""

RUN if [ -n "$FEATURES" ]; then \
        echo "Building with features: $FEATURES" \
        && cargo build --release --no-default-features --features "$FEATURES"; \
    elif [ -n "$CARGO_FLAGS" ]; then \
        echo "Building with flags: $CARGO_FLAGS" \
        && cargo build --release $CARGO_FLAGS; \
    else \
        cargo build --release; \
    fi

# Compile locales
RUN if [ -d "data/po" ]; then \
        for po in data/po/*.po; do \
            [ -f "$po" ] || continue; \
            lang="$(basename "$po" .po)"; \
            out="target/pot-gtk.mo/$lang/LC_MESSAGES"; \
            mkdir -p "$out"; \
            msgfmt "$po" -o "$out/pot-gtk.mo"; \
        done; \
    fi

# ─── Deb packager ────────────────────────────────────────────────────────────
FROM builder AS deb-packager
ARG FEATURES=""

RUN rm -rf target/pot-gtk.mo \
    && for po in data/po/*.po; do \
        [ -f "$po" ] || continue; \
        lang="$(basename "$po" .po)"; \
        out="target/pot-gtk.mo/$lang/LC_MESSAGES"; \
        mkdir -p "$out"; \
        msgfmt "$po" -o "$out/pot-gtk.mo"; \
    done \
    && if [ -n "$FEATURES" ]; then \
        cargo deb --no-default-features --features "$FEATURES"; \
    else \
        cargo deb; \
    fi

# ─── RPM packager ─────────────────────────────────────────────────────────
FROM builder AS rpm-packager
ARG FEATURES=""

RUN if [ -n "$FEATURES" ]; then \
        cargo build --release --no-default-features --features "$FEATURES"; \
    else \
        cargo build --release; \
    fi \
    && if [ -d "data/po" ]; then \
        for po in data/po/*.po; do \
            [ -f "$po" ] || continue; \
            lang="$(basename "$po" .po)"; \
            out="target/pot-gtk.mo/$lang/LC_MESSAGES"; \
            mkdir -p "$out"; \
            msgfmt "$po" -o "$out/pot-gtk.mo"; \
        done; \
    fi \
    && cargo generate-rpm

# ─── AppImage packager ───────────────────────────────────────────────────
FROM builder AS appimage-packager
ARG FEATURES=""

RUN apt-get update && apt-get install -y --no-install-recommends \
    file \
    fuse \
    libfuse2 \
    wget \
    && rm -rf /var/lib/apt/lists/*

RUN if [ -n "$FEATURES" ]; then \
        cargo build --release --no-default-features --features "$FEATURES"; \
    else \
        cargo build --release; \
    fi

# Download linuxdeploy
RUN wget -q "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage" \
        -O /usr/local/bin/linuxdeploy \
    && chmod +x /usr/local/bin/linuxdeploy

WORKDIR /src/appimage-build

# Assemble AppDir
RUN mkdir -p AppDir/usr/bin \
    && cp /src/target/release/pot-gtk AppDir/usr/bin/ \
    && chmod +x AppDir/usr/bin/pot-gtk \
    && mkdir -p AppDir/usr/share/applications \
    && cp /src/data/com.pot-app.pot-gtk.desktop AppDir/ \
    && cp /src/data/com.pot-app.pot-gtk.desktop AppDir/usr/share/applications/ \
    && mkdir -p AppDir/usr/share/icons/hicolor/scalable/apps \
    && cp /src/data/icons/scalable/apps/com.pot-app.pot-gtk.svg AppDir/usr/share/icons/hicolor/scalable/apps/ \
    && mkdir -p AppDir/usr/share/icons/hicolor/128x128/apps \
    && cp /src/data/icons/128x128/apps/com.pot-app.pot-gtk.png AppDir/usr/share/icons/hicolor/128x128/apps/ \
    && mkdir -p AppDir/usr/share/icons/hicolor/64x64/apps \
    && cp /src/data/icons/64x64/apps/com.pot-app.pot-gtk.png AppDir/usr/share/icons/hicolor/64x64/apps/ \
    && mkdir -p AppDir/usr/share/icons/hicolor/48x48/apps \
    && cp /src/data/icons/48x48/apps/com.pot-app.pot-gtk.png AppDir/usr/share/icons/hicolor/48x48/apps/ \
    && mkdir -p AppDir/usr/share/icons/hicolor/32x32/apps \
    && cp /src/data/icons/32x32/apps/com.pot-app.pot-gtk.png AppDir/usr/share/icons/hicolor/32x32/apps/ \
    && mkdir -p AppDir/usr/share/icons/hicolor/16x16/apps \
    && cp /src/data/icons/16x16/apps/com.pot-app.pot-gtk.png AppDir/usr/share/icons/hicolor/16x16/apps/ \
    && if [ -d /src/target/pot-gtk.mo ]; then \
        cp -r /src/target/pot-gtk.mo/* AppDir/usr/share/locale/ 2>/dev/null || true; \
    fi

# Build the AppImage
RUN OUTPUT="Pot-GTK-x86_64.AppImage" linuxdeploy --appdir AppDir --output appimage \
    && mv Pot-GTK-x86_64.AppImage /src/target/Pot-GTK-x86_64.AppImage
