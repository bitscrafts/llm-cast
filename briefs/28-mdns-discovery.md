# Feature Brief — Task #28: Chromecast mDNS Discovery

## Context

`cast-tv-terminal` displays a herdr session on a Chromecast. The device is
currently hardcoded via `CAST_DEVICE` env var (default `10.10.10.208`, port
8009). This is a "container networking fix" task.

Goal: **auto-discover the Chromecast on the LAN via mDNS/DNS-SD** instead of
relying on a hardcoded IP, so the pipeline finds the device even when its IP
changes. Fall back to the explicit `CAST_DEVICE` when discovery fails.

## Existing surface

- `src/mcp/config.rs` — `Config.cast_device: String` (env `CAST_DEVICE`,
  default `10.10.10.208`).
- `src/mcp/cast.rs` — `production_cast_port(device: String)` builds a
  `CastPort` closure; uses `cast::sender::DeviceAddr` + `cast::session::send_media_load`.
- `src/cast/sender.rs` — `DeviceAddr::new(ip)`.
- `src/bin/castctl.rs` — takes `<device-ip> <url>` directly.

## Desired behavior

1. Add a discovery step that queries mDNS for the `_googlecast._tcp` service
   (Chromecast's DNS-SD type) and resolves the device's IP:port.
2. If exactly one Chromecast is found, use it (overriding the hardcoded IP).
   If none found, fall back to `CAST_DEVICE`. If multiple, use the first or
   prefer the one matching `CAST_DEVICE` if provided.
3. Expose the discovery result so `pipeline_status`/operators can see which
   device was resolved (e.g. a `discovered_device` field or log line).
4. Must not break the existing explicit-IP path or the castctl smoke test.

## Constraints

- Rust-first, match existing style (tokio async, thiserror errors).
- mDNS via a crate (e.g. `mdns-sd`, `bon`, or a manual DNS-SD query). Prefer a
  maintained crate with no heavy system deps (avoiding the gstreamer/cast
  optional-gate situation).
- Discovery is best-effort: any discovery failure MUST fall back to the
  configured `CAST_DEVICE`, never error the whole pipeline.
- Tests under `_tmp/`, no network required for unit tests (mock the resolver).

## Deliverable

A spec part (`specs/04-mdns-discovery.md`) with Overview, Requirements
(Functional/Non-Functional), Architecture, TDD Contract, Exit Criteria
(machine-checkable `- [ ]` + backticked command), Guardrails, Files to Modify.
