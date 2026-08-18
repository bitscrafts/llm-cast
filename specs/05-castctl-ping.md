# chromecast-tv-mirror — spec-05: device override for mirror binary

- **Project**: `/projects/chromecast-tv-mirror`
- **Priority**: small greenfield task to fairly test the local workhorse
- **Status**: SPECIFIED — not yet dispatched
- **Source**: local workhorse fair-test (2026-08-18)
- **Depends-On**: none (self-contained, new bin file)

---

## Verified Premises

- `src/bin/castctl.rs` — operator smoke-test binary, takes `<device-ip> <url>`.
- `src/cast/sender.rs:25-41` — `DeviceAddr { host, port }`, `DeviceAddr::new(host)` → port 8009.
- No existing "list devices" or "device override" helper in `src/bin/`.

---

## Overview

Add a small, self-contained helper binary `src/bin/castctl.rs` enhancement: a
`--ping` flag that, given a device IP, attempts a TCP connect to port 8009 and
reports whether the Chromecast is reachable. This is a greenfield, single-file
addition — ideal for fairly testing whether a local workhorse (laguna) writes
real code.

---

## Requirements

### Functional

- **R1 (ping flag)**: `castctl --ping <device-ip>` attempts a TCP connection to
  `<ip>:8009` with a 3-second timeout. Prints `reachable` (exit 0) or
  `unreachable: <err>` (exit 1).
- **R2 (no url needed)**: `--ping` does NOT require the `<url>` positional —
  it works standalone.

### Non-Functional

- **N1**: no unwrap/expect/panic in the new code path.
- **N2**: no new dependencies (use `std::net::TcpStream`).
- **N3**: gate green; `cargo build --bin castctl` compiles.

---

## Architecture

`castctl --ping <ip>` → `TcpStream::connect_timeout((ip, 8009), 3s)` → print
result, exit 0/1. No changes to existing binaries or libs.

**What this spec is not**: no mDNS, no config, no changes to `mirror.rs` or the
MCP server.

---

## TDD Contract

Tests in a new `tests/castctl_tests.rs` (or inline). Since `castctl` is a bin,
the reachability logic should be a small testable function.

| id | test | given | expects |
|----|------|-------|---------|
| R1 | `test_ping_format_reachable` | host that connects | function returns `true`, prints "reachable" |
| R1 | `test_ping_format_unreachable` | unroutable IP (e.g. 127.0.0.1:1) | function returns `false`, prints "unreachable" |
| R1 | `test_ping_connect_timeout_bounded` | connect to an unroutable IP | completes within ~5s (bounded, not hanging) |

**Acceptance**: `castctl --ping 127.0.0.1` reports reachable (localhost); the
exit code matches reachable/unreachable.

---

## Exit Criteria

- [ ] `cargo build --bin castctl` — compiles (N3)
- [ ] `cargo test --test castctl_tests 2>&1 | grep -qE "^test result: ok\. [1-9]"` — tests ran (R1)
- [ ] `cargo test` — whole suite passes (N3)
- [ ] `grep -q 'connect_timeout' src/bin.rs` — the ping uses a bounded connect (R1)
- [ ] `cargo clippy --all-targets -- -D warnings` — clean (N3)
- [ ] `cargo fmt -- --check` — formatted (N3)
- [ ] `! grep -rn 'unwrap()\|expect(\|panic!' src/bin/castctl.rs` — no panics (N1)

**Prose criteria:**
1. `castctl --ping <ip>` works standalone without `<url>`.
2. Test counts pasted raw, one per binary, unsummed.

---

## Guardrails

- **G1 — do NOT edit this spec.** If wrong, reply `SPEC-DEFECT: <summary>`.
- **G2 — do NOT commit.**
- **G3 — do NOT weaken/skip/delete an existing test.**
- **G5 — no hardcoded absolute paths.**
- **G6 — report raw output, never summed totals.**
- **G7 — do NOT change `mirror.rs`, `mcp-server.rs`, or lib code.**

### Error handling expectations

- `--ping` on an unreachable host must exit 1, not panic.
- The connect must be timeout-bounded (3s), never hang.

---

## Files to Modify

| File | Change |
|------|--------|
| `src/bin/castctl.rs` | add `--ping` flag + reachability function + tests (R1,R2) |

**Not modified**: `mirror.rs`, `mcp-server.rs`, `src/lib.rs`, `specs/*`, `.orchestration/`.
