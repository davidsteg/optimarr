#!/bin/sh
set -e

# Runs as root, makes the data and log folders writable for PUID:PGID, then drops privileges.
if [ "$(id -u)" = "0" ]; then
    PUID="${PUID:-1000}"
    PGID="${PGID:-1000}"
    if [ "$(id -g optimarr)" != "$PGID" ]; then groupmod -o -g "$PGID" optimarr; fi
    if [ "$(id -u optimarr)" != "$PUID" ]; then usermod -o -u "$PUID" optimarr; fi
    for dir in "$OPTIMARR_DATA_DIR" "$OPTIMARR_LOG_DIR"; do
        [ -n "$dir" ] || continue
        mkdir -p "$dir"
        chown -R optimarr:optimarr "$dir" 2>/dev/null || true
    done
    exec gosu optimarr "$@"
fi
exec "$@"
