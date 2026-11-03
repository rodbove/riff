#!/bin/sh
set -e
cd "$(dirname "$0")/.."

case "$(uname -s)" in
Darwin)
    ./scripts/bundle.sh
    DEST=/Applications
    [ -w "$DEST" ] || DEST="$HOME/Applications"
    mkdir -p "$DEST"
    rm -rf "$DEST/riff.app"
    cp -R target/riff.app "$DEST/riff.app"
    echo "installed $DEST/riff.app"
    ;;
Linux)
    cargo build --release -p riff-app
    BIN="$HOME/.local/bin"
    APPS="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
    mkdir -p "$BIN" "$APPS"
    install -m 755 target/release/riff "$BIN/riff"
    cat > "$APPS/riff.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=riff
GenericName=Guitar Amp
Comment=Guitar amp and effects for metal
Exec=$BIN/riff
Icon=audio-card
Terminal=false
Categories=AudioVideo;Audio;Music;
Keywords=guitar;amp;metal;effects;
DESKTOP
    command -v update-desktop-database >/dev/null && update-desktop-database "$APPS" || true
    echo "installed $BIN/riff and the riff menu entry"
    case ":$PATH:" in
    *":$BIN:"*) ;;
    *) echo "note: $BIN is not in PATH. Start riff from the app menu, or add it to PATH." ;;
    esac
    if [ "$1" = "--plugins" ]; then
        cargo xtask bundle riff-plugin --release
        mkdir -p "$HOME/.clap" "$HOME/.vst3"
        rm -rf "$HOME/.clap/riff.clap" "$HOME/.vst3/riff.vst3"
        cp -R target/bundled/riff.clap "$HOME/.clap/"
        cp -R target/bundled/riff.vst3 "$HOME/.vst3/"
        echo "installed the CLAP and VST3 plugins in ~/.clap and ~/.vst3"
    fi
    ;;
*)
    echo "unsupported system: $(uname -s)" >&2
    exit 1
    ;;
esac
