# Container dependency installs (for the container file)

> The container root `/` is overlayfs — everything installed via `apt-get`
> below is **ephemeral** and flushes on reboot. Append these to the container
> file (Containerfile/Dockerfile) to persist them. Logged 2026-08-16 during the
> milestone-2 operator test.

## GStreamer (real encode leg — `gstreamer` feature of `cast_tv_terminal`)

Needed to compile `--features gstreamer` (the `gstreamer-sys`/`-app-sys`/
`-base-sys`/`-video-sys` crates need `pkg-config` to find these `.pc` files)
and to run the encode pipeline at runtime.

```dockerfile
RUN apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
    libgstreamer1.0-dev \
    libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-base \
    gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad \
    gstreamer1.0-plugins-ugly \
    gstreamer1.0-tools
```

Notes:
- `gstreamer1.0-plugins-bad` provides **`hlssink2`** (the HLS muxer used by
  `GstEncoder`; `hlsmux` is NOT a real element).
- `gstreamer1.0-plugins-ugly` provides **`x264enc`** (H.264 encoder).
- `gstreamer1.0-plugins-base` provides `videoconvert`.
- `gstreamer1.0-tools` provides `gst-inspect-1.0` (verification).
- There is NO `gstreamer1.0-x264` package — x264enc ships in `-plugins-ugly`.
- The Rust crates need the `.pc` files: `gstreamer-1.0`, `gstreamer-base-1.0`,
  `gstreamer-app-1.0`, `gstreamer-video-1.0` (from `libgstreamer1.0-dev` +
  `libgstreamer-plugins-base1.0-dev`).
- `vaapih264enc` (the `--encoder vaapi` path) additionally needs the Intel VA-API
  driver stack (`gstreamer1.0-vaapi` + `intel-media-driver`/`intel-vaapi-driver`),
  **not** installed in this container — the x264 path is the default.

## openssh-client (operator SSH to the podman host)

Needed to reach the podman host (`lnx`, 10.10.10.217) from inside the container to
set up the LAN-reachability forward. The host's SSH is reachable from the container
at `host.containers.internal` (169.254.1.2), port 22 — NOT at `10.10.10.217:22`
(closed/filtered on the LAN side from inside the bridge). Logged 2026-08-16.

```dockerfile
RUN apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq \
    openssh-client
```

Notes:
- Client-only; the host side must be set up once per host reboot (see below).

---

## Host-side (podman host `lnx`, 10.10.10.217) — LAN reachability forward

The Chromecast (10.10.10.208) cannot reach the container's private bridge
(`10.89.0.2`) — it can only reach the host's LAN IP `10.10.10.217`, and the host
only publishes the container ports created with `podman run -p`. Adding a new
published port would require recreating the container (not allowed mid-run). So the
host runs a **userspace TCP forward** that needs `socat` on the host:

```bash
# on the host (as root, or sudo):
apt-get install -y socat
socat TCP-LISTEN:18080,fork,reuseaddr,bind=10.10.10.217 TCP:10.89.0.2:18080 &
```

- `bind=10.10.10.217` — only the LAN interface (the Chromecast's path).
- `TCP:10.89.0.2:18080` — the container's bridge address + the `mirror` HLS port.
- Userspace relay: the running container is never touched (no recreate, no restart).
- **Ephemeral on the host** — a `&`-backgrounded process dies with the shell; for
  durability add it to the host's init/compose, e.g.:
  ```yaml
  # compose `services.mirror-forward` on the HOST (not the container):
  image: alpine/socat:latest
  command: TCP-LISTEN:18080,fork,reuseaddr,proxyport=10.89.0.2 TCP:10.89.0.2:18080
  network_mode: host
  ```

<!-- More installs get appended here as the milestone-2 run needs them. -->
