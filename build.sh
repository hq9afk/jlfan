#!/usr/bin/env bash

# Build script for jlfan
# Use:
# <empty>: release build, then the tests
# setup: install build and runtime dependencies, and add the user to uucp for /dev/ttyACM* access
# build: release build only
# install: build and install system-wide (DESTDIR=dir stages into dir without sudo)
# uninstall: stop the user service and remove everything install placed (honours DESTDIR)
# run: install, then (re)start the user service

set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"

BIN=jlfan
DESTDIR="${DESTDIR:-}"
DATA=/usr/share/jlfan
UNIT=/usr/lib/systemd/user/jlfan.service
ASSETS=(
    test.gif
    hue.gif
    bar.gif
    pulse.gif
    blink.gif
)

as_root() { if [[ -z "$DESTDIR" ]]; then sudo "$@"; else "$@"; fi; }

cmd_setup() {
    sudo pacman -Syu --needed rust ffmpeg
    sudo usermod -aG uucp "$USER"
}

cmd_build() { cargo build --release; }

cmd_default() { cmd_build; cargo test --release; }

cmd_install() {
    cmd_build
    as_root install -Dm755 "target/release/$BIN" "$DESTDIR/usr/bin/$BIN"
    local asset
    for asset in "${ASSETS[@]}"; do
        as_root install -Dm644 "misc/$asset" "$DESTDIR$DATA/$asset"
    done
    as_root install -Dm644 misc/jlfan.service "$DESTDIR$UNIT"
}

cmd_uninstall() {
    [[ -n "$DESTDIR" ]] || systemctl --user disable --now jlfan 2>/dev/null || true
    as_root rm -rf "$DESTDIR/usr/bin/$BIN" "$DESTDIR$DATA" "$DESTDIR$UNIT"
}

cmd_run() {
    systemctl --user disable jlfan 2>/dev/null || true # drops an old `enable <path>` link that would shadow $UNIT
    cmd_install
    systemctl --user daemon-reload
    systemctl --user enable jlfan
    systemctl --user restart jlfan
}

main() {
    local cmd="${1:-default}"
    case "$cmd" in
    default | setup | build | install | uninstall | run) shift || true; "cmd_$cmd" "$@" ;;
    *) echo "unknown command: $cmd" >&2; exit 2 ;;
    esac
}

main "$@"
