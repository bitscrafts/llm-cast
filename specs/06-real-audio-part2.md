# chromecast-tv-mirror — spec-06 part 2: mirror --audio-source CLI wiring

- **Project**: `/projects/chromecast-tv-mirror`
- **Priority**: part 2 of spec-06 (real audio). Wires the `--audio-source` flag
  through the `mirror` CLI into the encoder seam built in part 1.
- **Status**: SPECIFIED
- **Source**: split of `06-real-audio.md`
- **Depends-On**: part 1 (`src/encode/pipe.rs` audio seam)

---

## Verified Premises

- `src/bin/mirror.rs:80-110` — the CLI arg parse loop matches `--source`,
  `--bind`, `--size`, etc.; no `--audio-source` exists.
- `src/bin/mirror.rs:218` — `GstEncoder::new(&encoder, width, height, FPS,
  &outdir, &root, url.clone())` — no audio param yet (part 1 adds it).
- `src/bin/mirror.rs:246-251` — the non-gstreamer (`NullEncoder`) path ignores
  encoder options; `--audio-source` there must be accepted-but-inert.
- `src/bin/mirror.rs:50-61` — the `--help` text lists the flags.

---

## Overview

The encoder seam (part 1) accepts an optional audio source. This part exposes
it to the operator: `mirror --audio-source '<launch-fragment>'` sets the audio
leg. With no flag, silent AAC is preserved (DMR mandate).

---

## Requirements

### Functional

- **R1 (flag)**: `mirror` accepts `--audio-source <fragment>` and stores it.
  Missing value → usage error (exit 2), consistent with `--source`.
- **R2 (wire to encoder)**: with the `gstreamer` feature, the audio fragment
  is passed to `GstEncoder::new(..., audio: Option<&str>)`.
- **R3 (silent default)**: with no `--audio-source`, `None` is passed → silent
  AAC (part 1's R2 default).
- **R4 (inert without gstreamer)**: on a default-features build (NullEncoder),
  `--audio-source` is parsed but ignored (no crash). Documented in `--help`.

### Non-Functional

- **N1**: `--help` shows `--audio-source <fragment>`.
- **N2**: no new Rust dependencies.
- **N3**: does NOT touch `src/encode/pipe.rs` (part 1) or `src/serve/`,
  `src/cast/`, `src/capture/`, `src/render/`, `src/emu/`, `src/pipeline/`.

---

## Architecture

```
mirror --audio-source 'filesrc location=/tmp/a.wav ! decodebin ! audioconvert ! audioresample' ...
       │
       ▼
parse loop → audio: Option<String>
       │
       ▼
gstreamer feature? ──yes──→ GstEncoder::new(..., audio.as_deref())
       │
       no (NullEncoder)
       ▼
       ignore (documented)
```

**Key decision — a free-form launch fragment.** The flag takes any
GStreamer-valid launch fragment. The encoder (part 1) concatenates it. No
extra parsing, no fixed source types. Rejected: discrete `--audio-file` /
`--audio-device` flags (a fragment is strictly more general and needs no
enum).

**Key decision — accept-but-inert on NullEncoder.** Default-features builds
have no GStreamer; the flag must not crash. It is parsed, ignored, and
documented.

---

## TDD Contract

| id | test | given | expects |
|----|------|-------|---------|
| R1 | `test_mirror_accepts_audio_source_flag` | `mirror --help` output | contains `--audio-source` |
| R1 | `test_mirror_missing_audio_value_errors` | `mirror --source x --audio-source` | exit code 2, error message |
| R3 | `test_mirror_no_audio_passes_none` | `mirror --source x --no-cast` (no --audio-source) | encoder gets `audio=None` (silent default) |
| R4 | `test_mirror_audio_inert_no_gstreamer` | default-features `mirror --source x --audio-source ... --no-cast` | runs without panic; flag ignored |

**Acceptance: R1 `test_mirror_accepts_audio_source_flag`.** A plausible
implementation adds the flag but forgets `--help`, or forgets the
accept-but-inert behavior on NullEncoder. The flag must appear in `--help`.

---

## Exit Criteria

- [ ] `cd /projects/chromecast-tv-mirror && cargo test 2>&1 | grep -qE "^test result: ok\. [1-9]"` — suite passes
- [ ] `cd /projects/chromecast-tv-mirror && cargo test --features gstreamer test_mirror_accepts_audio_source_flag 2>&1 | grep -qE "^test result: ok\. [1-9]"` — R1 acceptance
- [ ] `cd /projects/chromecast-tv-mirror && cargo clippy -- -D warnings` — clean
- [ ] `cd /projects/chromecast-tv-mirror && cargo run --bin mirror -- --help 2>&1 | grep -q "\-\-audio-source"` — live `--help` shows the flag
- [ ] `cd /projects/chromecast-tv-mirror && git diff --quiet -- specs/` — no spec edits (G1)

**Prose criteria**:

1. Raw `--help` output pasted showing `--audio-source`.
2. `src/bin/mirror.rs` shows the parse arm and the `audio.as_deref()` forward
   to `GstEncoder::new`.

---

## Guardrails

- **G1 — do NOT edit this spec.**
- **G2 — do NOT commit.**
- **G3 — do NOT weaken/skip/delete an existing test.**
- **G4 — no hardcoded absolute paths.**
- **G5 — no `rm -rf` of run state.**
- **G6 — report raw output, never summed totals.**
- **G7 — do NOT touch `src/encode/pipe.rs` (part 1) or any module outside
  `src/bin/mirror.rs` and its tests.**

---

## Files to Modify (part 2 only)

| File | Change |
|------|--------|
| `src/bin/mirror.rs` | add `--audio-source` parse arm, forward to encoder (R1-R4) |
| `tests/` (or `tests/cast_tv_tests.rs`) | R1/R3/R4 tests |

**Not modified**: `specs/`, `src/encode/`, everything else.
