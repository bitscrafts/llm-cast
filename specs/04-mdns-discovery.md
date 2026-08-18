# chromecast-tv-mirror — spec-04: Chromecast mDNS Discovery

- **Project**: `/projects/chromecast-tv-mirror`
- **Priority**: removes the hardcoded `CAST_DEVICE` IP dependency so the
  pipeline finds the Chromecast on the LAN even when its DHCP lease changes;
  best-effort with explicit-IP fallback
- **Status**: SPECIFIED · *(lifecycle: SPECIFIED → IN PROGRESS on dispatch →
  IMPLEMENTED — awaiting review → DONE after the orchestrator commits and
  updates HANDOFF.md)*
- **Source**: `briefs/28-mdns-discovery.md` — "container networking fix" task
- **Depends-On**: spec-03 part 1 (`src/mcp/cast.rs::production_cast_port`,
  `src/mcp/config.rs::Config.cast_device`, `src/mcp/errors::McpServerError`,
  `src/mcp/status.rs::pipeline_status_json`) and spec-01 (`src/cast::DeviceAddr`)

---

## Verified Premises

<Re-checked in the tree on 2026-08-18.>

- `src/mcp/config.rs:27,104` — `Config.cast_device: String`, env `CAST_DEVICE`,
  default `10.10.10.208`. The single source of the hardcoded device today.
- `src/mcp/cast.rs:55-85` — `production_cast_port(device: String) -> CastPort`
  builds the closure; the `cast`-feature body constructs
  `DeviceAddr::new(device.clone())` per call and calls `send_media_load`.
  Without `cast` it is a stub `Err`. The `device` string is captured by the
  closure.
- `src/cast/sender.rs:25-41` — `DeviceAddr { host, port: u16 }`,
  `DeviceAddr::new(host)` sets port `8009`. The Chromecast Cast v2 TLS port.
- `src/cast/sender.rs:48` — `pub type Discovery = Box<dyn FnMut() ->
  Result<DeviceAddr, CastError>>` (already used by `Sender`). This spec adds a
  *resolver* abstraction (returns a host string, best-effort, never fatal)
  rather than reusing the panicky-on-miss `Discovery` box.
- `src/mcp/status.rs:52-88` — `pipeline_status_json` emits a flat JSON object
  with `mux`, `processes`, `resolution`, `session`, `display`, `hls`. A new
  top-level `cast` object will carry `configured_device`, `discovered_device`,
  and `source`.
- `src/bin/mcp-server.rs:47` — the bin wires `production_cast_port(
  config.cast_device.clone())`. This is the integration point: the bin will
  instead build a resolver and pass it in.
- `src/bin/mirror.rs:281-313` — `cast_to(ip, url)` builds
  `Sender::new(Box::new(move || Ok(DeviceAddr::new(ip.clone()))))`. The mirror
  binary takes `--device IP` directly from the operator and is NOT changed by
  this spec (the operator-supplied explicit IP already overrides discovery;
  G7 keeps it untouched).
- `src/bin/castctl.rs` — takes `<device-ip> <url>` on the CLI. NOT changed
  (operator smoke-test; G7).
- `Cargo.toml:7-44` — deps + features; `default = []`, optional `cast`,
  optional `gstreamer`. `[[test]] mcp_tests` already declared. The only new
  dep this spec adds is the mDNS crate (N2).
- `tests/cast_tv_tests.rs:650-716` — `test_no_production_unwrap` walks 8 dirs
  (`capture,emu,render,encode,serve,cast,mcp,mux`) and asserts
  `checked_dirs.len() == 8`. A new `src/cast/discovery.rs` file is inside the
  already-walked `src/cast` dir, so the count stays 8 — the meta-test needs NO
  edit (G6).
- `src/mcp/mod.rs:8-14` — submodules `cast,config,display,errors,runner,
  sizing,status`. No `discovery` submodule; the resolver lives in
  `src/cast/discovery.rs` (cast-side concern) and is re-exported from
  `src/cast/mod.rs`.
- Crate `mdns-sd` (maintained, pure-Rust, no system avahi/bonjour deps): the
  DNS-SD browser API `ServiceDaemon::new()?.browse("_googlecast._tcp",
  recv)`. It polls a `Receiver<ServiceEvent>`; `ServiceEvent::ServiceFound`
  then `ServiceResolved(_service, addr)` yields `ServiceInfo { addresses:
  Vec<IpAddr>, port: u16, ... }`. Confirmed maintained as of 2026; no heavy
  native deps (satisfies the brief's "avoid the gstreamer/cast optional-gate
  situation").

---

## Overview

Today every cast path targets a single hardcoded IP (`CAST_DEVICE`, default
`10.10.10.208`). When the Chromecast's DHCP lease changes the pipeline silently
points at a dead address. This spec adds a **best-effort mDNS/DNS-SD discovery
step** that browses `_googlecast._tcp` on the LAN, resolves the device's
IP:port, and uses it in place of the hardcoded IP. Discovery is **never fatal**:
any failure (no devices, timeout, mDNS socket error) falls back to
`Config.cast_device` so the existing explicit-IP path is unchanged.

The resolver is a trait (`DeviceResolver`) with two implementations: a real
`MdnsResolver` (behind a feature so the default build needs no network) and a
`StaticResolver` (returns the configured IP, used as the fallback and in tests).
`production_cast_port` gains an optional resolver argument; when discovery
succeeds the resolved host overrides the configured host, and the choice is
surfaced in `pipeline_status` as `cast.discovered_device` + `cast.source` so an
operator can see which device was selected.

**Scope is one self-contained part.** It adds: the resolver trait + impls
(`src/cast/discovery.rs`), the mDNS dependency (gated), wiring through
`production_cast_port` and the mcp-server bin, the `cast` status block, and the
unit tests (mocked resolver — no network). The mirror/castctl operator binaries
are explicitly untouched (G7).

---

## Requirements

### Functional

- **R1 (resolver trait)**: `src/cast/discovery.rs` defines
  `pub trait DeviceResolver: Send + Sync` with
  `fn resolve(&self) -> Result<DiscoveredDevice, DiscoveryError>`.
  `DiscoveredDevice { host: String, port: u16, source: DiscoverySource }`
  where `DiscoverySource` is an enum `{ Mdns, Config }`. The trait is
  object-safe (`dyn DeviceResolver`) so it can be injected and mocked.
- **R2 (StaticResolver / fallback)**: `StaticResolver { host: String }`
  implements `DeviceResolver` and always returns
  `Ok(DiscoveredDevice { host, port: 8009, source: Config })`. This is the
  configured-IP path and the universal fallback.
- **R3 (MdnsResolver, feature-gated)**: `MdnsResolver` implements
  `DeviceResolver` by browsing `_googlecast._tcp` via the `mdns-sd` crate,
  waiting up to `MDNS_TIMEOUT_SECS` (env, default 3) for at least one
  `ServiceResolved`. Selection:
  - exactly one device → use it;
  - multiple → prefer the one whose resolved address matches `CAST_DEVICE` if
    set; otherwise the first resolved;
  - none / timeout / socket error → `Err(DiscoveryError)`.
  The whole impl is `#[cfg(feature = "mdns")]`; without the feature the type
  does not exist (N2).
- **R4 (best-effort compose)**: `pub fn resolve_device(
  config_device: &str) -> DiscoveredDevice` runs `MdnsResolver` (when the
  feature is on) and on ANY `Err` falls back to `StaticResolver`. It NEVER
  returns `Err` and NEVER panics — discovery failure is always the configured
  IP. A `log::warn!` records the fallback reason.
- **R5 (cast port wiring)**: `production_cast_port` gains the resolved device.
  The closure captures a `DiscoveredDevice` (host + port) and builds
  `DeviceAddr { host, port }` per call instead of `DeviceAddr::new(device)`.
  The existing one-arg `production_cast_port(device: String)` signature is
  kept as a thin wrapper that calls `resolve_device(&device)` then the new
  two-arg form — so `mirror.rs`/`castctl.rs` and existing tests that call the
  one-arg form are unchanged (G7, N5).
- **R6 (status surfacing)**: `pipeline_status_json` adds a top-level `cast`
  object: `{ "configured_device": <Config.cast_device>,
  "discovered_device": <host:port or null>, "source": "mdns"|"config"|"unknown" }`.
  When the cast port has no resolved device yet (e.g. built without `cast`),
  `discovered_device` is `null` and `source` is `"unknown"`. The field is
  always present (never absent) so a client can rely on it.

### Non-Functional

- **N1 (no-unwrap)**: no `.unwrap()`/`.expect()`/`panic!()` in
  `src/cast/discovery.rs`. The `test_no_production_unwrap` meta-test already
  walks `src/cast`, so it covers the new file with NO edit to the count (still
  8 dirs) — G6 forbids weakening it, and none is needed.
- **N2 (dependency)**: the ONLY new dependency is
  `mdns-sd = { version = "0.11", optional = true }`, plus a new feature
  `mdns = ["dep:mdns-sd"]` in `[features]`. No other Cargo.toml change. The
  spec-02 rustix `std` pin stays (G9). The default build (`default = []`)
  pulls in NO mDNS code and needs no network.
- **N3 (gates stay green)**: `cargo build`, `cargo build --features cast`,
  `cargo build --features cast,gstreamer`, `cargo build --features mdns`,
  `cargo build --features cast,mdns`, `cargo test`, `cargo clippy
  --all-targets -- -D warnings`, `cargo fmt -- --check` — all pass. Test count
  only goes up.
- **N4 (best-effort invariant)**: discovery failure MUST NOT error the
  pipeline. `resolve_device` returns `Ok` unconditionally; the cast-port
  closure and `pipeline_status` never propagate a discovery error. The only
  errors that surface from the cast leg are the existing `send_media_load`
  session errors (unchanged).
- **N5 (no regression)**: `production_cast_port(device: String)` one-arg form
  still exists with the same behavior for a caller that passes a literal IP
  (it resolves to `source: Config` when `mdns` is off, or attempts mDNS then
  falls back when `mdns` is on). `mirror.rs` and `castctl.rs` are NOT edited.
- **N6 (no hardcoded absolute paths)**: every runtime value (timeout, service
  type) comes from env or a `const` in the module; no `/projects/…` or
  `/root/…` literals in production code. Test artefacts under `_tmp/` or env
  temp.
- **N7 (no network in unit tests)**: every test uses `StaticResolver` or a
  `FakeResolver` (mocked `DeviceResolver`). No test browses the real LAN; no
  test needs `--features mdns` to run (the `MdnsResolver` impl is exercised
  only by a compile-check exit criterion, not a runtime test).

---

## Architecture

```mermaid
flowchart TD
    CFG["Config.cast_device (env CAST_DEVICE)"] --> RES["resolve_device()"]
    RES -->|"#[cfg(feature=\"mdns\)]"| MDNS["MdnsResolver: browse _googlecast._tcp"]
    MDNS -->|"found"| PICK["pick: match CAST_DEVICE else first"]
    MDNS -->|"none/timeout/err"| FALL["log::warn! -> StaticResolver"]
    RES -->|"!feature mdns"| FALL
    FALL --> STATIC["StaticResolver: Config IP:8009"]
    PICK --> DD["DiscoveredDevice { host, port, source }"]
    STATIC --> DD
    DD --> CP["production_cast_port: closure captures DD"]
    CP --> SESS["send_media_load(DeviceAddr{host,port})"]
    DD --> ST["pipeline_status: cast.{configured,discovered,source}"]
```

**Key decision — a resolver trait, not a `Discovery` box.** `src/cast/sender.rs`
already has `pub type Discovery = Box<dyn FnMut() -> Result<DeviceAddr,
CastError>>`, but it is panicky-on-miss and returns `DeviceAddr` directly. The
brief requires best-effort fallback, so this spec adds `DeviceResolver` (returns
`DiscoveredDevice` with a `source` tag, never fatal). The two coexist:
`Sender`'s `Discovery` stays for the mirror binary; `DeviceResolver` is the
MCP-server path. **Rejected:** reusing `Discovery` — its `Err` would propagate
into the cast port and error the pipeline, violating N4.

**Key decision — feature-gate the mDNS impl.** `mdns-sd` opens a UDP multicast
socket; that is undesirable in the default build and unavailable in many test
sandboxes. The `MdnsResolver` type and the `mdns-sd` dep are behind
`feature = "mdns"`. `resolve_device` is compiled in all builds: with `mdns` off
it is just `StaticResolver` (one branch). With `mdns` on it tries `MdnsResolver`
then falls back. This keeps `cargo test` (default features) network-free (N7).

**Key decision — `DiscoveredDevice` carries `port`.** `DeviceAddr::new` hardcodes
8009. mDNS advertises the real port (8009 for Chromecast, but the DNS-SD record
is authoritative). Resolving to `host:port` and building `DeviceAddr { host,
port }` is strictly more correct than ignoring the advertised port.

**What this spec is not**: no change to `mirror.rs`/`castctl.rs` (operator
binaries take an explicit IP and already override discovery), no change to the
`Sender`/`Discovery` types in `sender.rs`, no change to the mux layer, no
change to `send_media_load`, no live-device test. The `MdnsResolver` is
feature-gated and unit-tested only by compilation (N7).

### Resolver seam (the shapes the tests depend on)

```rust
/// Where the device address came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiscoverySource { Mdns, Config }

/// A resolved Chromecast address + provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredDevice {
    pub host: String,
    pub port: u16,
    pub source: DiscoverySource,
}

/// Best-effort Chromecast resolver. Never fatal: fall back to the configured IP.
pub trait DeviceResolver: Send + Sync {
    fn resolve(&self) -> Result<DiscoveredDevice, DiscoveryError>;
}

/// Always returns the configured IP:8009 (the fallback and the non-mdns path).
pub struct StaticResolver { pub host: String }

/// mDNS/DNS-SD browser for `_googlecast._tcp`. Feature-gated.
#[cfg(feature = "mdns")]
pub struct MdnsResolver {
    pub config_device: String,   // preferred match if set
    pub timeout_secs: u64,
}

/// The one call the cast port makes. Never returns Err.
pub fn resolve_device(config_device: &str) -> DiscoveredDevice;
```

`production_cast_port` new shape:

```rust
pub fn production_cast_port_with(device: DiscoveredDevice) -> CastPort { ... }
pub fn production_cast_port(device: String) -> CastPort {
    production_cast_port_with(resolve_device(&device))
}
```

### Config (env, all with defaults)

`CAST_DEVICE` (10.10.10.208) — existing. **New:** `MDNS_TIMEOUT_SECS` (3) —
seconds `MdnsResolver` waits for a resolve before falling back. `MDNS_SERVICE`
(`_googlecast._tcp`) — the DNS-SD service type, overridable for tests/future
devices. Both read inside `MdnsResolver` construction (only when the `mdns`
feature is on).

---

## TDD Contract

Tests live in the existing `tests/mcp_tests.rs` target (no new `[[test]]`).
A `FakeResolver` (mocked `DeviceResolver`) scripts `Ok(DiscoveredDevice)` /
`Err(DiscoveryError)` results; no test enables the `mdns` feature or touches
the network (N7).

| id | test | given | expects |
|----|------|-------|---------|
| R2 | `test_static_resolver_returns_config` | `StaticResolver { host: "10.10.10.208" }` | `Ok(DiscoveredDevice { host:"10.10.10.208", port:8009, source:Config })` |
| R1,R4 | `test_resolve_device_falls_back_on_err` | `FakeResolver` returning `Err(DiscoveryError::Timeout)`, `resolve_device` wired to use it then `StaticResolver` | `Ok` with `source: Config` and the configured host; never `Err`; never panic |
| R4 | `test_resolve_device_uses_mdns_result` | `FakeResolver` returning `Ok(DiscoveredDevice{host:"192.168.1.55", port:8009, source:Mdns})` | `resolve_device` returns that device unchanged |
| R5 | `test_production_cast_port_uses_resolved_host` (`#[cfg(not(feature="cast"))]`) | `production_cast_port_with(DiscoveredDevice{host:"9.9.9.9", port:8009, source:Mdns})` then call the port | the stub `Err` message (no cast feature) — proves the closure captured the resolved device without panicking; with `#[cfg(feature="cast")]` the test instead asserts the `DeviceAddr` built inside matches by inspecting a seam (see note) |
| R6 | `test_pipeline_status_has_cast_block` | `McpServer` with a `StaticResolver`-resolved device wired into the cast port | `pipeline_status_json()` parses; JSON has `cast.configured_device == "10.10.10.208"`, `cast.discovered_device` is `"10.10.10.208:8009"`, `cast.source == "config"` |
| R6 | `test_pipeline_status_cast_block_when_no_device` | `McpServer` whose cast port has no resolved device (built without `cast`) | `cast.discovered_device` is `null`, `cast.source == "unknown"`; the `cast` object is present |
| N1 | `test_no_production_unwrap` (existing, unedited) | the meta-test walks `src/cast` (incl. new `discovery.rs`) | still `checked_dirs.len() == 8`; passes with zero offenders in the new file |

**R5 cast-feature note.** Under `#[cfg(feature="cast")]` the real closure calls
`send_media_load`, which opens a TLS socket — not unit-testable. The test
asserts the *seam*: `production_cast_port_with` stores the `DiscoveredDevice`
in an inspectable field exposed for tests (`pub(crate) fn last_device(&self) ->
&DiscoveredDevice` on the closure's owner), OR the test runs only under
`#[cfg(not(feature="cast"))]` and asserts the stub error string contains the
resolved host. The implementer picks whichever is cleaner; the contract is
"the resolved host reaches the closure", proved by the stub message under the
default (no-cast) build.

**Acceptance test — `test_resolve_device_falls_back_on_err` (R4).** The obvious
implementation propagates the resolver's `Err` out of `resolve_device`, which
would error the pipeline on a quiet LAN. This test only passes if
`resolve_device` catches the `Err`, logs, and returns the `StaticResolver`
result — the invariant that makes discovery safe.

---

## Exit Criteria

- [ ] `cargo build` — default features compile (the `mdns`-gated code is absent) (N2)
- [ ] `cargo build --features mdns` — the mDNS resolver compiles against `mdns-sd` (R3,N2)
- [ ] `cargo build --features cast,mdns` — cast + mdns compile together (N3)
- [ ] `cargo test` — whole suite incl. `mcp_tests` passes (N3)
- [ ] `cargo test --test mcp_tests 2>&1 | grep -qE "^test result: ok\. [1-9]"` — the new resolver/status tests actually ran (non-vacuous) (R1-R6)
- [ ] `test -f src/cast/discovery.rs && grep -q "pub trait DeviceResolver" src/cast/discovery.rs && grep -q "pub fn resolve_device" src/cast/discovery.rs` — the resolver trait + entrypoint exist (R1,R4)
- [ ] `grep -q 'mdns-sd' Cargo.toml && grep -q '^mdns = ' Cargo.toml` — dependency + feature declared (N2)
- [ ] `grep -q 'production_cast_port_with' src/mcp/cast.rs && grep -q '"cast"' src/mcp/status.rs` — cast-port wiring + status block exist (R5,R6)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean (N3)
- [ ] `cargo fmt -- --check` — formatted (N3)
- [ ] `! grep -rn 'unwrap()\|expect(\|panic!' src/cast/discovery.rs` — no panicking calls in the new module (N1)
- [ ] `! git diff --name-only | grep -qE 'src/bin/(mirror|castctl)\.rs'` — operator binaries untouched (N5,G7)
- [ ] `! git diff --name-only | grep -qE 'specs/(01|02|03)-'` — prior specs untouched (G1)

**Prose criteria:**

1. `resolve_device` is provably total: paste its body and confirm every branch
   returns `Ok` (the `Err` from `MdnsResolver` is caught and replaced).
2. Test counts pasted raw, one line per binary, **unsummed**.
3. Confirm `cargo build --features mdns` adds `mdns-sd` to the lockfile WITHOUT
   removing the spec-02 rustix `std` pin (G9).

---

## Guardrails

- **G1 — do NOT edit this spec.** If it is wrong, reply with a first line of
  `SPEC-DEFECT: <summary>`.
- **G2 — do NOT commit.** Leave work in the working tree.
- **G3 — do NOT weaken, skip or delete an existing test.**
- **G4 — do NOT edit `tests/cast_tv_tests.rs`'s `checked_dirs.len()` (still 8).**
  The new `src/cast/discovery.rs` is inside the already-walked `src/cast` dir.
- **G5 — no hardcoded absolute paths in production code.** Test artefacts under
  env temp or `_tmp/`.
- **G6 — report raw output, never summed totals.**
- **G7 — do NOT edit `src/bin/mirror.rs` or `src/bin/castctl.rs`.** The operator
  binaries take an explicit IP and already override discovery; they are out of
  scope. Their existing `production_cast_port`/`DeviceAddr::new` calls must keep
  compiling unchanged (N5).
- **G8 — do NOT touch the operator's live default herdr session, Xvfb, ffmpeg,
  hls_server, or the running cycle loop.** Tests never talk to the live stack
  and never browse the real LAN (N7).
- **G9 — do NOT run `cargo add`.** Edit `Cargo.toml` directly; keep the spec-02
  rustix `std` pin. If `mdns-sd`'s rustix tree conflicts with the pin, STOP and
  report — never remove the pin.
- **G10 — do NOT install system packages** (avahi, bonjour, ...). `mdns-sd` is
  pure-Rust; no system daemon is needed.
- **G11 — discovery is best-effort, always.** `resolve_device` must never
  return `Err` and never `panic!`. Any mDNS failure → `log::warn!` +
  `StaticResolver`. This is the acceptance invariant (R4/N4).

### Error handling expectations

- `MdnsResolver::resolve` may return `Err(DiscoveryError::{Timeout, Socket,
  NoDevices})` — these are EXPECTED and caught by `resolve_device`.
- `StaticResolver::resolve` is infallible (returns `Ok`).
- `resolve_device` is infallible (returns `Ok`).
- The cast-port closure's only `Err` path is the existing `send_media_load`
  session error (unchanged) or the no-`cast`-feature stub — never a discovery
  error.
- `pipeline_status` `cast` block is always present; `discovered_device` is
  `null` only when no device has been resolved (e.g. no cast port wired).

---

## Files to Modify

| File | Change |
|------|--------|
| `Cargo.toml` | add `mdns-sd = { version = "0.11", optional = true }` to `[dependencies]`; add `mdns = ["dep:mdns-sd"]` to `[features]` (N2) |
| `src/cast/mod.rs` | add `pub mod discovery;` and `pub use discovery::{DeviceResolver, DiscoveredDevice, DiscoverySource, StaticResolver, resolve_device};` (R1) |
| `src/cast/discovery.rs` | **NEW** — `DiscoveryError` (thiserror), `DiscoverySource`, `DiscoveredDevice`, `DeviceResolver` trait, `StaticResolver`, `#[cfg(feature="mdns")] MdnsResolver`, `resolve_device()` (R1-R4) |
| `src/mcp/cast.rs` | add `production_cast_port_with(DiscoveredDevice) -> CastPort`; refactor the existing `production_cast_port(device: String)` to call `resolve_device(&device)` then the new form; the closure builds `DeviceAddr { host, port }` from the resolved device (R5) |
| `src/mcp/status.rs` | add a top-level `cast` object to `pipeline_status_json` with `configured_device`, `discovered_device`, `source`; read the resolved device from the server's cast port seam (R6) |
| `src/mcp/mod.rs` | add a `pub(crate)` field on `McpServer` holding the resolved `DiscoveredDevice` (or expose it from the cast port) so `status` can report it (R6) |
| `src/bin/mcp-server.rs` | unchanged behaviorally — `production_cast_port(config.cast_device.clone())` still works (it now resolves internally). Optionally log the resolved device at startup. (R5,N5) |
| `tests/mcp_tests.rs` | add `FakeResolver`, the resolver tests, and the two `pipeline_status` cast-block tests (R1-R6) |

**Not modified**: `src/bin/mirror.rs`, `src/bin/castctl.rs`, `src/cast/sender.rs`,
`src/cast/session.rs`, `src/mcp/runner.rs`, `src/mcp/config.rs`, `src/mux/**`,
`tests/cast_tv_tests.rs`, `specs/01-*`, `specs/02-*`, `specs/03-*`,
`.orchestration/`, `HANDOFF.md`, `docs/`.