#!/bin/sh
# M2M hardened-image entrypoint.
# Starts Tor in the background (for optional SOCKS5 use from M2M's
# private-mode settings), then runs M2M.
#
# NOTE: M2M is a desktop GUI app. Run with a Wayland/X11 socket mounted
# (see README.md). For headless/server use, M2M's listener still works for
# LAN peers; the UI simply isn't rendered.

tor --quiet &
TOR_PID=$!

# Give Tor a moment to bootstrap its port.
i=0
while [ $i -lt 20 ] && ! nc -z 127.0.0.1 9050 2>/dev/null; do
    i=$((i + 1))
    sleep 0.5
done

exec /usr/local/bin/m2m "$@"
