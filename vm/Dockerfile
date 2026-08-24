# M2M Hardened VM Image (Debian + Tor)
#
# Builds a minimal container/VM rootfs with M2M and Tor preconfigured.
# Roadmap §4 "Ship hardened VM image". See README.md in this directory for
# usage and threat-model notes.
#
# Build:  docker build -t m2m-hardened -f vm/Dockerfile .
# Run:    see vm/README.md

FROM debian:bookworm-slim

ENV DEBIAN_FRONTEND=noninteractive

# Minimal footprint: tor + runtime libs only. No shell tools beyond what's
# needed; no compiler in the final image (supply-chain surface reduction).
RUN apt-get update && apt-get install -y --no-install-recommends \
        tor \
        ca-certificates \
        libwebkit2gtk-4.1-0 \
        libgtk-3-0 \
        libayatana-appindicator3-1 \
        librsvg2-common \
    && rm -rf /var/lib/apt/lists/*

# Tor: SocksPort only (no relays, no control port), discard logs.
COPY torrc /etc/tor/torrc

# M2M binary built OUTSIDE the image by scripts/build-release.sh and
# verified against published SHA-256 hashes before inclusion.
COPY m2m /usr/local/bin/m2m
COPY m2m-entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh /usr/local/bin/m2m \
    && useradd --create-home --shell /usr/sbin/nologin m2muser \
    && mkdir -p /home/m2muser/.m2m /run/tor \
    && chown -R m2muser:m2muser /home/m2muser \
    && chown debian-tor:debian-tor /run/tor

USER m2muser
WORKDIR /home/m2muser

ENTRYPOINT ["/entrypoint.sh"]
