# Spec: cast-tv-terminal — Part 5/5

**Parent-Spec**: `01-cast-tv-terminal.md`
**Part**: 5 of 5
**Covers**: R6 (the REAL `media_load` onto the device via rust_cast — the
part-2 stub's replacement), R11 (safe degrade), plus the `castctl` operator
binary for the milestone-1 device smoke test.
**Status**: SPECIFIED — WRITTEN 2026-08-16. Amends part-2's `Discovery` seam
(was `Result<(), CastError>`; the real session has nothing to connect to —
now `Result<DeviceAddr, CastError>`). IMPLEMENTED 2026-08-16 — gate GREEN,
review PASS, validate 5/5, EXIT 0, no exit-7 violation. The implement pass
corrected three code-level API sketches (verified against rust_cast 0.17.0
source — the notes said to verify, not guess): `CastDeviceApp::from_str`
(no `From<&str>` impl); `DeviceAddr::new(ip.clone())` (FnMut E0507); and
`media.load`'s destination is the launched app's `transport_id`, both args
`&String`. Behavior contract unchanged.

## Overview

Parts 1-4 shipped the pipeline with the cast sender's device session as a
stub behind `#[cfg(feature = "cast")]`. This part implements that session for
real: given a discovered device address, connect with rust_cast (TLS), launch
the Default Media Receiver (`CC1AD845`), and send the Cast v2 media LOAD
carrying the HLS URL. All of it is feature-gated — the crate still compiles
and tests green under default features. A tiny `castctl` binary gives the
operator a one-liner to run the milestone-1 smoke test on the device.

Milestone-1 device (operator-provided): Chromecast at `10.10.10.208`,
MAC `54:60:09:DE:4D:24` (2026-08-16).

## Modules in this part

```
src/
├── cast/
│   ├── mod.rs         MODIFY: `pub use sender::{CastError, DeviceAddr, Sender};`
│   ├── sender.rs      MODIFY: Discovery type amended; session moved out
│   └── session.rs     (NEW)  `#[cfg(feature = "cast")]` real rust_cast session
└── bin/
    └── castctl.rs     (NEW)  operator smoke-test binary
```

## Key Data Structures (owned/amended by this part)

```rust
/// LAN address of a discovered Chromecast (listens on 8009, TLS).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceAddr {
    pub host: String,
    pub port: u16,
}
impl DeviceAddr {
    /// `host` at the standard Chromecast port 8009.
    pub fn new(host: impl Into<String>) -> Self { ... }
}

/// AMENDED from part 2: discovery must hand back WHERE the device is, or the
/// session has nothing to connect to. Still injectable, still fails fast.
pub type Discovery = Box<dyn FnMut() -> Result<DeviceAddr, CastError>>;
```

`CastError`, `Sender::new`, `build_media_load_request`, `Sender::send_load`
stay exactly as part 2 pinned them (payload `{"type":"LOAD",...}` unchanged;
`test_sender_reports_unreachable` still passes — a discovery returning `Err`
still propagates `CastError::Unreachable` with no network and no hang).

## TDD Contract

| Test Name | Given | Expects |
|-----------|-------|---------|
| `test_sender_reports_unreachable` (existing, must still pass) | injected discovery returns `Err` | `send_load(url)` returns `Err(CastError::...Unreachable)` promptly, no hang |
| `test_sender_accepts_device_address` (NEW) | injected discovery returns `Ok(DeviceAddr { host: "192.168.1.50", port: 8009 })` | `send_load(url)` returns `Ok(())` under default features (session compiled out; no network touched) — proves the amended seam composes end-to-end |
| `test_device_addr_default_port` (NEW) | `DeviceAddr::new("10.0.0.5")` | `.port == 8009` and `.host == "10.0.0.5"` |

## Implementation Notes

- **Discovery seam (amend part 2)**: change `Discovery` to return
  `Result<DeviceAddr, CastError>`. The two existing sender tests keep
  compiling — the always-fail closure's `Err` infers the new result type. The
  existing `test_sender_reports_unreachable` must pass UNCHANGED.
- **`send_load` flow** (all features):
  ```text
  device = (self.discovery)()?            // -> DeviceAddr, or CastError
  [feature "cast"] self.session_media_load(&device, url)?
  Ok(())                                  // default features: session compiled out
  ```
  Under default features the discovered `device` must NOT trigger an unused
  variable warning (clippy -D warnings runs in the gate) — handle with a
  `let _ = &device;` under `#[cfg(not(feature = "cast"))]` or equivalent.
- **`src/cast/session.rs`** — `#[cfg(feature = "cast")]` module, no new deps
  (rust_cast is already optional). Real flow (verify each call against
  `rust_cast-0.17.0` source; error type is `rust_cast::Error`):
  1. `CastDevice::connect(&device.host, device.port)` (TLS; if cert
     verification blocks a real device, use
     `connect_without_host_verification` — the device's self-signed certs are
     expected). Map every `rust_cast::Error` to our `CastError::Session` via
     `map_err` — never `.unwrap()`.
  2. Wire the channels from the connected device's message manager:
     `ConnectionChannel` (`connect("receiver-0")`), `HeartbeatChannel`
     (ping), `ReceiverChannel::launch_app(&"CC1AD845".into())` → returns an
     `Application` carrying `session_id`.
  3. `MediaChannel::load(destination, &session_id, &Media { content_id: url,
     stream_type: StreamType::Live, content_type:
     "application/x-mpegURL" })` — the fields mirror
     `build_media_load_request(url)` exactly (that function stays the pure
     payload spec; rust_cast's `load` builds the wire message from these
     fields).
  Channel construction details (which handles/senders the channels need) are
  in the crate source — READ it, don't guess. The session is
  `pub fn send_load(device: &DeviceAddr, url: &str) -> Result<(), CastError>`.
- **`src/bin/castctl.rs`** — operator smoke-test binary:
  - Args: `<device-ip> <hls-url>` (hls-url required; device port 8009).
  - Builds `Sender::new(Box::new(move || Ok(DeviceAddr::new(ip))))`, calls
    `send_load(url)`, prints a clear PASS/FAIL line.
  - Under default features print `"built without the cast feature — no session
    will be sent; rebuild with --features cast"`; under the feature print the
    device + url being loaded. Compiles both ways (cargo test builds bins).
  - No `.unwrap()`/`.expect()`/`panic!()` outside `fn main`'s test-like use —
    prefer `if let Ok(...)`, `?` + `eprintln!`, and a non-zero exit code on
    failure.
- **Build deps**: rust_cast 0.17 needs `openssl-sys` ⇒ `pkg-config` +
  `libssl-dev` at build time. These are installed in this container
  (2026-08-16). The smoke-test host needs them too. `cargo check --features
  cast` MUST pass — that is exit criterion 2.
- The part-4 fail-closed sweep only walks `src/capture src/emu src/render
  src/encode src/serve src/cast` — `src/bin` is out of scope for it, but this
  part's own criterion 5 covers `src/bin`.

## Exit Criteria

- [ ] `cargo test --test cast_tv_tests 2>&1 | grep -q "test result: ok"`
- [ ] `cargo check --features cast 2>&1 | grep -q "Finished"`
- [ ] `test -f src/cast/session.rs && test -f src/cast/sender.rs && test -f src/bin/castctl.rs`
- [ ] `grep -q "CC1AD845" src/cast/session.rs`
- [ ] `! grep -rE '\.unwrap\(\)|\.expect\(' src/cast src/bin 2>/dev/null | grep -v '//' | grep -v '#\[cfg\(test\)\]' | grep -v test`

## Guardrails

- Do not run `cargo`, `rustc`, `clippy` outside the TDD cycle steps
- Do not add public API surface not specified in Requirements
- Do not use `.unwrap()`, `.expect()`, or `panic!()` in production code paths
- Do not modify files outside the project root
- Do not add dependencies to `Cargo.toml` — rust_cast is already an approved
  optional dep; nothing new is needed
- **Approved dependencies**: `alacritty_terminal`, `rust_cast` (optional),
  `gstreamer*` (optional), `tiny-skia`, `tokio`, `axum`, `tower-http`,
  `serde`, `serde_json`, `thiserror`
- **PIVOT GUARDRAIL**: do NOT build a custom Cast receiver, register with
  Google Cast, or use WebRTC for the first milestone. If `rust_cast` cannot
  `media_load` HLS onto the device, STOP and report for an explicit Option-2
  decision. `castctl` exists so the operator can answer that question; it is
  not a license to improvise a receiver.

On any ambiguity, stop and report back, do not guess.
