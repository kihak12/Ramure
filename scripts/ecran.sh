#!/usr/bin/env bash
# Lance Ramure sur un écran virtuel et en prend des captures, sous Linux, sans écran réel.
# Sert à donner des captures aux subagents design-reviewer et ux-designer.
#
#   scripts/ecran.sh ouvrir <clair|sombre> [dépôt…]   lance l'application sur ces dépôts
#   scripts/ecran.sh capturer <fichier.png>            capture la fenêtre
#   scripts/ecran.sh fermer                            arrête l'application et l'écran virtuel
#
# Entre « ouvrir » et « capturer », l'application se pilote avec xdotool, sur l'écran :99 :
#   DISPLAY=:99 xdotool mousemove 760 138 click 1
#
# Il faut Xvfb, xdotool, ImageMagick, python3, et l'application compilée
# (npm run tauri build -- --no-bundle). L'application tourne avec un dossier personnel à part :
# ni les réglages ni le carnet d'hôtes de l'utilisateur ne sont touchés.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
bin=$root/src-tauri/target/release/ramure
home=${TMPDIR:-/tmp}/ramure-ecran
size=1440x880 # la taille de la fenêtre, dans src-tauri/tauri.conf.json
export DISPLAY=:99

stop() {
  pkill -x ramure 2>/dev/null || true
}

case ${1:-} in
  ouvrir)
    theme=${2:?clair ou sombre}
    shift 2
    [ -x "$bin" ] || { echo "Application non compilée : npm run tauri build -- --no-bundle" >&2; exit 1; }
    stop
    if ! pgrep -x Xvfb >/dev/null; then
      Xvfb :99 -screen 0 1500x920x24 >/dev/null 2>&1 &
      sleep 1
    fi
    # Les onglets à rouvrir sont lus dans le stockage local de la page : on y écrit les dépôts.
    rm -rf "$home"
    mkdir -p "$home/.local/share/dev.ramure.app/localstorage"
    python3 - "$home/.local/share/dev.ramure.app/localstorage/tauri_localhost_0.localstorage" "$@" <<'P'
import json, os, sqlite3, sys
db = sqlite3.connect(sys.argv[1])
db.execute("CREATE TABLE ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB NOT NULL ON CONFLICT FAIL)")
paths = [os.path.abspath(p) for p in sys.argv[2:]]
if paths:
    tabs = json.dumps({"paths": paths, "active": 0})
    db.execute("INSERT INTO ItemTable VALUES (?, ?)", ("ramure.tabs", tabs.encode("utf-16-le")))
db.commit()
P
    [ "$theme" = sombre ] && gtk=Adwaita:dark || gtk=Adwaita
    HOME=$home XDG_CONFIG_HOME=$home/.config XDG_DATA_HOME=$home/.local/share \
      GTK_THEME=$gtk WEBKIT_DISABLE_DMABUF_RENDERER=1 \
      dbus-run-session -- "$bin" >"$home/ramure.log" 2>&1 &
    timeout 30 xdotool search --sync --onlyvisible --name '^Ramure$' >/dev/null
    sleep 4 # le temps que la page charge les dépôts
    ;;
  capturer)
    out=${2:?fichier de sortie}
    mkdir -p "$(dirname "$out")"
    import -window root -crop "$size+0+0" +repage "$out"
    ;;
  fermer)
    stop
    pkill -x Xvfb 2>/dev/null || true
    ;;
  *)
    sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'
    exit 1
    ;;
esac
