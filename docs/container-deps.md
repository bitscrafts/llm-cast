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

**Topology (learned the hard way, 2026-08-16):** `pidag-runner` is **rootless
podman** (`podman info` → `rootless=true`). Its `10.89.0.2` is a *virtual*
address with **no route from the host** — the host cannot reach the container by
IP at all (`connect 10.89.0.2:18080` → "Connection timed out"), and the container
can only reach *published* host ports (`-p`). So a plain host-side socat →
container-IP forward **cannot work**. Only two mechanisms bridge the container:

1. **Durable fix — publish the port** in `podman-compose.yml`:
   ```yaml
   ports:
     - "4601:4601"
     - "8080:8080"  # llama-server (Laguna local inference)
     - "18080:18080"  # mirror HLS (chromecast-tv-mirror milestone-2)
   ```
   Takes effect on the **next container recreate** (does not disturb a running
   container). This is what's committed to the podman files.

2. **Mid-run fix (container stays up) — reverse SSH tunnel + host socat.**
   The container *can* reach the host (`host.containers.internal` =
   `169.254.1.2`), so the container initiates a tunnel and the host socat
   bridges LAN 18080 → the tunneled loopback port:
   ```bash
   # host: bridge the LAN port to the tunnel (needs socat on the host)
   sudo apt-get install -y socat
   setsid nohup socat TCP-LISTEN:18080,fork,reuseaddr,bind=0.0.0.0 \
       TCP:127.0.0.1:18081 >> /tmp/mirror-forward.log 2>&1 &

   # container: reverse tunnel, host 127.0.0.1:18081 -> mirror 18080
   setsid nohup ssh -N -o ExitOnForwardFailure=yes -o ServerAliveInterval=30 \
       -o ServerAliveCountMax=3 -R 127.0.0.1:18081:127.0.0.1:18080 \
       mvcorrea@169.254.1.2 &
   ```
   Path: `TV → 10.10.10.217:18080 → host socat → 127.0.0.1:18081 → SSH → mirror`.
   sshd binds the `-R` port on loopback only (`GatewayPorts no`), hence the socat
   hop. Firewall is permissive (iptables INPUT ACCEPT), so LAN clients pass.
   These two processes are ephemeral; both survive the launching shell
   (`setsid nohup`) but not a host/container reboot.

> ⚠️ Do NOT `pkill -f "socat TCP-LISTEN:18080"` inside a command that *also*
> starts/contains the socat string — the pattern matches your own command line
> and kills your own SSH session (exit 144). Kill in a separate invocation, or
> use a non-self-matching regex like `1808[0]`.

<!-- More installs get appended here as the milestone-2 run needs them. -->
