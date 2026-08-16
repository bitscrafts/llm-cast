#!/bin/bash
# Bring up the chromecast-tv-mirror TV display stack (herdr tv-demo session).
#
# The TV display runs its OWN herdr session with the PROJECT's herdr config
# (config/herdr/config.toml), never the operator's ~/.config/herdr/config.toml.
# HERDR_CONFIG_PATH points the tv-demo server + client at the project config
# (sidebar pinned narrow + collapsed to the compact rail for the TV).
#
# Prereqs (already running, do not kill): Xvfb :99, ffmpeg x11grab->HLS,
# hls_server.py :18080, the reverse tunnel. Only the tv-demo herdr server,
# the display client (xterm) and the pane programs are (re)created here.
#
# Usage: scripts/tv-demo-up.sh

set -u
REPO=/projects/chromecast-tv-mirror
PROJECT_CONFIG=$REPO/config/herdr/config.toml
SOCK=/root/.config/herdr/sessions/tv-demo/herdr.sock
DISPLAY=:99

# --- env strip: a herdr child must not inherit HERDR_* (nested-herdr guard) ---
STRIP=(env -u HERDR_ENV -u HERDR_PANE_ID -u HERDR_SOCKET_PATH -u HERDR_TAB_ID \
       -u HERDR_WORKSPACE_ID)

echo "==> herdr tv-demo server (project config)"
if [ -S "$SOCK" ]; then
  old_pid=$(pgrep -f 'herdr server' | xargs -I{} sh -c \
    "grep -q HERDR_SESSION=tv-demo /proc/{}/environ 2>/dev/null && echo {}" | head -1)
  if [ -n "$old_pid" ]; then
    echo "    stopping existing tv-demo server pid=$old_pid"
    kill "$old_pid"
    sleep 1
  fi
fi
"${STRIP[@]}" DISPLAY=$DISPLAY HERDR_SESSION=tv-demo \
  HERDR_STARTUP_CWD=/projects/chromecast-tv-mirror \
  HERDR_CONFIG_PATH="$PROJECT_CONFIG" \
  /root/.local/bin/herdr server >/tmp/m2/herdr-server.log 2>&1 &
sleep 2

echo "==> display client (xterm -> herdr --session tv-demo)"
# NOTE: xterm must be a detached background job (run_in_background in the
# orchestrator); a bare `&` here gets reaped when this script exits.
nohup "${STRIP[@]}" DISPLAY=$DISPLAY xterm -class XTerm \
  -fa 'DejaVu Sans Mono' -fs 12 -geometry 126x35+0+0 \
  -xrm 'XTerm*scrollBar: false' -xrm 'XTerm*menuBar: false' \
  -xrm 'XTerm*internalBorder: 0' -xrm 'XTerm*background: black' \
  -xrm 'XTerm*foreground: white' -T herdr-tv \
  -e /bin/sh -c "exec herdr --session tv-demo" \
  >>/tmp/m2/xterm-tv.log 2>&1 &
sleep 2

echo "==> pane programs (htop / watch lsof)"
"${STRIP[@]}" HERDR_SOCKET_PATH=$SOCK herdr pane run w1:p1 htop >/dev/null 2>&1
"${STRIP[@]}" HERDR_SOCKET_PATH=$SOCK herdr pane run w1:p2 'watch -n 2 lsof -i' >/dev/null 2>&1

echo "==> cycle loop (focus tab 1 / tab 2 every 10s) — if not already running"
if ! pgrep -f 'herdr tab focus w1:t1' >/dev/null; then
  nohup bash -c "while true; do HERDR_SOCKET_PATH=$SOCK herdr tab focus w1:t1 >/dev/null 2>&1; sleep 10; HERDR_SOCKET_PATH=$SOCK herdr tab focus w1:t2 >/dev/null 2>&1; sleep 10; done" \
    >>/tmp/m2/cycle.log 2>&1 &
fi

echo "==> done. Verify: HERDR_CONFIG_PATH=$PROJECT_CONFIG active on the tv-demo server."
