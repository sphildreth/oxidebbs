# syntax=docker/dockerfile:1

ARG FEDORA_VERSION=44

FROM fedora:${FEDORA_VERSION} AS builder

RUN dnf -y install \
      ca-certificates \
      clang \
      clang-devel \
      curl \
      gcc \
      git \
      glibc-devel \
      make \
      pkgconf-pkg-config \
      tar \
      xz \
    && dnf clean all

ENV RUSTUP_HOME=/root/.rustup \
    CARGO_HOME=/root/.cargo \
    PATH=/root/.cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --profile minimal

WORKDIR /build/oxidebbs

COPY rust-toolchain.toml Cargo.toml Cargo.lock ./
COPY crates ./crates

RUN cargo build --workspace --locked --release -p oxidebbs-server

FROM fedora:${FEDORA_VERSION} AS runtime

RUN dnf -y install \
      ca-certificates \
      coreutils \
      dnf-plugins-core \
      findutils \
      grep \
      procps-ng \
      sed \
      shadow-utils \
    && dnf -y copr enable stsp/dosemu2 \
    && dnf -y install \
      comcom64 \
      dj64dev-dj64 \
      dj64dev-djdev64 \
      dosemu2 \
      fdpp \
      install-freedos \
    && printf '%s\n' /usr/i386-pc-dj64/lib64 > /etc/ld.so.conf.d/dj64.conf \
    && ldconfig \
    && dnf clean all

RUN useradd --create-home --home-dir /home/oxidebbs --shell /bin/bash oxidebbs

WORKDIR /srv/oxidebbs

COPY --from=builder /build/oxidebbs/target/release/oxidebbs-server /usr/local/bin/oxidebbs-server
COPY assets /opt/oxidebbs/share/assets
COPY config /opt/oxidebbs/share/config
COPY scripts /opt/oxidebbs/share/scripts
COPY tools /opt/oxidebbs/share/tools
COPY scripts/docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

RUN mkdir -p \
      /srv/oxidebbs/assets \
      /srv/oxidebbs/config \
      /srv/oxidebbs/data \
      /srv/oxidebbs/doors \
      /srv/oxidebbs/logs \
      /srv/oxidebbs/runtime \
      /srv/oxidebbs/scripts \
      /srv/oxidebbs/tools \
    && chmod +x /usr/local/bin/docker-entrypoint.sh \
      /opt/oxidebbs/share/scripts/test-oxide-door-dosemu2.sh \
    && chown -R oxidebbs:oxidebbs /srv/oxidebbs /opt/oxidebbs /home/oxidebbs

USER oxidebbs

ENV OXIDEBBS_CONFIG=/srv/oxidebbs/config/oxidebbs.toml \
    OXIDEBBS_BOARD_NAME=OxideBBS \
    OXIDEBBS_SYSOP_ALIAS=sysop \
    OXIDEBBS_NODES=4 \
    OXIDEBBS_TELNET_PORT=2323 \
    OXIDEBBS_ENABLE_TEST_DOOR=1 \
    RUST_LOG=oxidebbs=info

EXPOSE 2323

ENTRYPOINT ["docker-entrypoint.sh"]
CMD ["serve"]
