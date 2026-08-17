#!/bin/bash
# Restart the chromecast display pipeline at a target resolution.
#
# Kills the existing Xvfb, ffmpeg x11grab->HLS and hls_server, then brings them
# back at WxH with every other parameter byte-identical to the live stack. The
# display xterm / herdr server / cycle loop are left untouched — re-anchor the
# xterm afterwards with the MCP mirror_session tool (it computes the font +
# geometry from TV_RESOLUTION/TV_TERMINAL/TV_MARGIN), then re-cast.
#
# This intentionally causes a brief TV blip (the operator approved "Apply now").
# The hls_server MUST stay on /tmp/m2/xhls (it chdirs there), so HLS_DIR is
# unchanged.
#
# Usage: scripts/display-up.sh [WxH]     (default 1280x800)

set -u
RES="${1:-1280x800}"
DISPLAY_NUM=:99
XHLS=/tmp/m2/xhls

# --- validate WxH (same grammar as sizing::Resolution::parse) ---
if ! [[ "$RES" =~ ^[0-9]+x[0-9]+$ ]]; then
  echo "usage: $0 [WxH]" >&2
  exit 2
fi
W="${RES%x*}"; H="${RES#*x}"
if [ "$W" -eq 0 ] || [ "$H" -eq 0 ]; then
  echo "resolution must be non-zero: $RES" >&2
  exit 2
fi

echo "==> display-up: restarting pipeline at $RES"

echo "==> stopping old pipeline (Xvfb / ffmpeg / hls_server)"
for pat in 'Xvfb :99' 'ffmpeg .*x11grab' 'hls_server.py'; do
  for pid in $(pgrep -f "$pat" 2>/dev/null); do
    echo "    kill $pid ($pat)"
    kill "$pid" 2>/dev/null || true
  done
done
sleep 1
for pat in 'Xvfb :99' 'ffmpeg .*x11grab' 'hls_server.py'; do
  for pid in $(pgrep -f "$pat" 2>/dev/null); do
    kill -9 "$pid" 2>/dev/null || true
  done
done
sleep 1

echo "==> Xvfb $DISPLAY_NUM -screen 0 ${RES}x24 -ac -nolisten tcp"
Xvfb "$DISPLAY_NUM" -screen 0 "${RES}x24" -ac -nolisten tcp \
  >/tmp/m2/xvfb.log 2>&1 &

echo "    waiting for Xvfb (xdpyinfo poll)..."
for i in $(seq 1 50); do
  if xdpyinfo -display "$DISPLAY_NUM" >/dev/null 2>&1; then
    break
  fi
  sleep 0.2
done
xdpyinfo -display "$DISPLAY_NUM" >/dev/null 2>&1 \
  || { echo "    Xvfb never came up on $DISPLAY_NUM" >&2; exit 1; }
echo "    Xvfb up: $(xdpyinfo -display "$DISPLAY_NUM" | grep -m1 dimensions | tr -s ' ')"

echo "==> clean stale segments"
rm -f "$XHLS"/live*.ts "$XHLS"/live.m3u8

echo "==> ffmpeg x11grab ${RES} -> HLS ($XHLS/live.m3u8)"
ffmpeg -y -loglevel error -f x11grab -video_size "$RES" -framerate 10 \
  -draw_mouse 0 -i "$DISPLAY_NUM" \
  -f lavfi -i anullsrc=channel_layout=stereo:sample_rate=44100 \
  -map 0:v -map 1:a -filter_threads 1 -threads 4 \
  -c:v libx264 -preset medium -tune zerolatency -pix_fmt yuv420p -crf 16 \
  -g 10 -sc_threshold 0 -deblock 0 -threads 4 \
  -c:a aac -b:a 128k -ar 44100 -ac 2 \
  -f hls -hls_time 1 -hls_list_size 6 -hls_flags delete_segments \
  -hls_base_url http://10.10.10.217:18080/ \
  "$XHLS/live.m3u8" >/tmp/m2/ffmpeg-hls.log 2>&1 &

echo "==> hls_server :18080 (must stay on $XHLS)"
cd "$XHLS" && nohup python3 /tmp/m2/hls_server.py >/tmp/m2/hls-server.log 2>&1 &
sleep 1

echo "==> done. verify:"
echo "    pgrep -af Xvfb   -> -screen 0 ${RES}x24"
echo "    pgrep -af ffmpeg -> -video_size ${RES}"
echo "    then re-anchor the xterm: mirror_session(\"default\") and cast_url."
