#!/usr/bin/env bash
# Lance Ramure sur un écran virtuel et en prend des captures, sous Linux, sans écran réel.
# Sert à donner des captures aux subagents design-reviewer et ux-designer.
#
#   scripts/ecran.sh ouvrir <clair|sombre> [dépôt…]   lance l'application sur ces dépôts
#   scripts/ecran.sh capturer <fichier.png>            capture la fenêtre
#   scripts/ecran.sh fermer                            arrête l'application et l'écran virtuel
#
#   ECRAN_TROUSSEAU=1 scripts/ecran.sh ouvrir …        avec un trousseau du système, vide et déverrouillé
#   ECRAN_TROUSSEAU=0 scripts/ecran.sh ouvrir …        sans aucun trousseau
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

# Attend qu'aucun processus de ce nom ne reste, 5 secondes au plus, puis l'arrête de force : un
# « fermer » suivi d'un « ouvrir » ne doit pas trouver l'écran ou l'application d'avant en train de
# s'éteindre.
wait_gone() {
  for _ in $(seq 50); do
    pgrep -x "$1" >/dev/null || return 0
    sleep 0.1
  done
  pkill -9 -x "$1" 2>/dev/null || true
}

stop() {
  pkill -x ramure 2>/dev/null || true
  wait_gone ramure
}

case ${1:-} in
  ouvrir)
    theme=${2:?clair ou sombre}
    shift 2
    [ -x "$bin" ] || { echo "Application non compilée : npm run tauri build -- --no-bundle" >&2; exit 1; }
    stop
    if ! pgrep -x Xvfb >/dev/null; then
      Xvfb :99 -screen 0 1500x920x24 >/dev/null 2>&1 &
      # L'écran répond quand xdotool peut en lire la taille.
      for _ in $(seq 50); do
        xdotool getdisplaygeometry >/dev/null 2>&1 && break
        sleep 0.1
      done
    fi
    # Les onglets à rouvrir sont lus dans le stockage local de la page : on y écrit les dépôts.
    # On ne vide que les données de l'application : le reste du dossier peut être tenu par le système.
    rm -rf "$home/.local/share/dev.ramure.app"
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
    # Le trousseau du système, pour les écrans qui retiennent un secret :
    #   ECRAN_TROUSSEAU=1  un trousseau (GNOME Keyring) déverrouillé, vide à chaque lancement ;
    #   ECRAN_TROUSSEAU=0  aucun trousseau : le bus de la session ne démarre aucun service ;
    #   sans la variable   ce que la machine a, comme avant.
    start='exec "$0"'
    bus=()
    case ${ECRAN_TROUSSEAU:-} in
      1)
        command -v gnome-keyring-daemon >/dev/null || { echo "Il manque gnome-keyring (apt install gnome-keyring)" >&2; exit 1; }
        rm -rf "$home/.local/share/keyrings"
        start='printf ramure | gnome-keyring-daemon --unlock --components=secrets >/dev/null 2>&1; exec "$0"'
        ;;
      0)
        # Un bus sans dossier de services : rien n'y est lancé à la demande, pas même un trousseau.
        cat >"$home/bus-sans-services.conf" <<'CONF'
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <keep_umask/>
  <listen>unix:tmpdir=/tmp</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
CONF
        bus=(--config-file="$home/bus-sans-services.conf")
        ;;
    esac
    HOME=$home XDG_CONFIG_HOME=$home/.config XDG_DATA_HOME=$home/.local/share \
      GTK_THEME=$gtk WEBKIT_DISABLE_DMABUF_RENDERER=1 \
      dbus-run-session "${bus[@]}" -- sh -c "$start" "$bin" >"$home/ramure.log" 2>&1 &
    timeout 30 xdotool search --sync --onlyvisible --name '^Ramure$' >/dev/null
    sleep 9 # le temps que la page charge les dépôts : un premier chargement est plus long
    ;;
  capturer)
    out=${2:?fichier de sortie}
    mkdir -p "$(dirname "$out")"
    import -window root -crop "$size+0+0" +repage "$out"
    ;;
  fermer)
    stop
    pkill -x Xvfb 2>/dev/null || true
    wait_gone Xvfb
    ;;
  *)
    sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'
    exit 1
    ;;
esac
