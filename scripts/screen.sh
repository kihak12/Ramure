#!/usr/bin/env bash
# Runs Ramure on a virtual screen and takes screenshots of it, on Linux, without a real display.
# It provides the screenshots for the design-reviewer and ux-designer subagents.
#
#   scripts/screen.sh open <light|dark> [repository…]  runs the application on these repositories
#   scripts/screen.sh capture <file.png>                captures the window
#   scripts/screen.sh close                             stops the application and the virtual screen
#
#   SCREEN_KEYRING=1 scripts/screen.sh open …           with a system keyring, empty and unlocked
#   SCREEN_KEYRING=0 scripts/screen.sh open …           with no keyring at all
#
# Between "open" and "capture", xdotool drives the application, on display :99:
#   DISPLAY=:99 xdotool mousemove 760 138 click 1
#
# It needs Xvfb, xdotool, ImageMagick, python3, and the application built
# (npm run tauri build -- --no-bundle). The application runs with a home of its own: neither the
# user's settings nor their host book are touched.
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
bin=$root/src-tauri/target/release/ramure
home=${TMPDIR:-/tmp}/ramure-screen
size=1440x880 # the size of the window, in src-tauri/tauri.conf.json
export DISPLAY=:99

# Waits until no process of this name is left, 5 seconds at most, then kills it: a "close"
# followed by an "open" must not find the previous screen or application still shutting down.
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
  open)
    theme=${2:?light or dark}
    shift 2
    [ -x "$bin" ] || { echo "Application not built: npm run tauri build -- --no-bundle" >&2; exit 1; }
    stop
    if ! pgrep -x Xvfb >/dev/null; then
      Xvfb :99 -screen 0 1500x920x24 >/dev/null 2>&1 &
      # The screen answers once xdotool can read its size.
      for _ in $(seq 50); do
        xdotool getdisplaygeometry >/dev/null 2>&1 && break
        sleep 0.1
      done
    fi
    # The tabs to reopen are read from the local storage of the page: the repositories go there.
    # Only the data of the application is cleared: the system may hold the rest of the folder.
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
    [ "$theme" = dark ] && gtk=Adwaita:dark || gtk=Adwaita
    # The system keyring, for the screens that keep a secret:
    #   SCREEN_KEYRING=1      an unlocked keyring (GNOME Keyring), empty at each launch;
    #   SCREEN_KEYRING=0      no keyring: the session bus starts no service;
    #   without the variable  whatever the machine has, as before.
    start='exec "$0"'
    bus=()
    case ${SCREEN_KEYRING:-} in
      1)
        command -v gnome-keyring-daemon >/dev/null || { echo "gnome-keyring is missing (apt install gnome-keyring)" >&2; exit 1; }
        rm -rf "$home/.local/share/keyrings"
        start='printf ramure | gnome-keyring-daemon --unlock --components=secrets >/dev/null 2>&1; exec "$0"'
        ;;
      0)
        # A bus with no service directory: nothing is started on demand there, not even a keyring.
        cat >"$home/bus-without-services.conf" <<'CONF'
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
        bus=(--config-file="$home/bus-without-services.conf")
        ;;
    esac
    HOME=$home XDG_CONFIG_HOME=$home/.config XDG_DATA_HOME=$home/.local/share \
      GTK_THEME=$gtk WEBKIT_DISABLE_DMABUF_RENDERER=1 \
      dbus-run-session "${bus[@]}" -- sh -c "$start" "$bin" >"$home/ramure.log" 2>&1 &
    timeout 30 xdotool search --sync --onlyvisible --name '^Ramure$' >/dev/null
    sleep 9 # time for the page to load the repositories: a first load takes longer
    ;;
  capture)
    out=${2:?output file}
    mkdir -p "$(dirname "$out")"
    import -window root -crop "$size+0+0" +repage "$out"
    ;;
  close)
    stop
    pkill -x Xvfb 2>/dev/null || true
    wait_gone Xvfb
    ;;
  *)
    sed -n '2,17p' "$0" | sed 's/^# \{0,1\}//'
    exit 1
    ;;
esac
