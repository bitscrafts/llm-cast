# chromecast-tv-mirror — spec-06 part 1: AudioSource seam + silent default

- **Project**: `/projects/chromecast-tv-mirror`
- **Priority**: foundation of spec-06 (real audio). Part 1 adds the `AudioSource`
  seam to the GStreamer encoder and pins the silent-AAC default (the DMR mandate).
- **Status**: SPECIFIED
- **Source**: split of `06-real-audio.md` after laguna stall (exit 6, test file too large for one pass)
- **Depends-On**: none

---

## Verified Premises

- `src/encode/pipe.rs:253-262` — `build_pipeline` hardcodes the audio leg:
  `audiotestsrc is-live=true wave=silence ! audioconvert ! audioresample !
  voaacenc bitrate=64000 ! aacparse ! hls.audio`. No way to feed real audio.
- `src/encode/pipe.rs:112` — `GstEncoder::new(encoder, width, height, fps,
  outdir, root, url)` has no audio parameter.
- `src/encode/pipe.rs:222` — `build_pipeline(encoder, width, height, fps,
  outdir, root)` same.
- The silent-AAC default is MANDATORY (HANDOFF 2026-08-16: DMR refuses
  video-only HLS). **This part pins it.**

---

## Overview

The encoder's audio leg is hardcoded silence. This part adds a **configurable
audio-source seam**: `build_pipeline` and `GstEncoder::new` accept an optional
audio source (a GStreamer launch fragment). `None` → the current silent AAC;
`Some(...)` → a real source. The default behavior is unchanged (N1).

---

## Requirements

### Functional

- **R1 (audio seam)**: `build_pipeline` gains a trailing `audio: Option<&str>`
  parameter (a GStreamer launch fragment, e.g.
  `filesrc location=/tmp/a.wav ! decodebin ! audioconvert ! audioresample`).
- **R2 (None → silent)**: when `audio` is `None`, the pipeline string uses
  `audiotestsrc is-live=true wave=silence ! audioconvert ! audioresample`
  (identical to today).
- **R3 (Some → real)**: when `audio` is `Some(frag)`, the pipeline string uses
  `frag ! audioconvert ! audioresample` instead of the silent source, and does
  NOT contain `audiotestsrc`.
- **R4 (GstEncoder passes through)**: `GstEncoder::new` gains an
  `audio: Option<&str>` param, forwarded to `build_pipeline`.
- **R5 (build errors)**: a malformed audio fragment surfaces as
  `EncodeError::Gst` (no panic) when the pipeline fails to build.

### Non-Functional

- **N1**: `audio=None` → the launch string is byte-identical to today
  (the acceptance test pins this).
- **N2**: no new Rust dependencies.
- **N3**: this part does NOT touch `mirror.rs` (the CLI wiring is part 2) and
  does NOT touch `tests/cast_tv_tests.rs` beyond the new tests it adds.

---

## Architecture

`build_pipeline` now builds the audio leg conditionally:

```
let audio_leg = match audio {
    None    => "audiotestsrc is-live=true wave=silence ! audioconvert ! audioresample",
    Some(f) => format!("{f} ! audioconvert ! audioresample"),
};
```

**Key decision — a launch fragment, not a plugin API.** The audio source is an
arbitrary GStreamer launch fragment the operator supplies. The encoder just
concatenates it. This keeps the change small and flexible. Rejected: a
`filesrc`-only struct (pulsesrc is equally valid and free via the same path).

**Key decision — acceptance is the silent default.** A plausible
implementation replaces the silent leg unconditionally, breaking the DMR
mandate. `R2`'s test pins the default.

---

## TDD Contract

| id | test | given | expects |
|----|------|-------|---------|
| R2 (acceptance) | `test_default_pipeline_has_silent_audio` | `build_pipeline(..., None, ...)` | launch string contains `audiotestsrc` and `wave=silence` |
| R3 | `test_pipeline_with_audio_source` | `build_pipeline(..., Some("filesrc location=/tmp/a.wav ! decodebin ! audioconvert ! audioresample"), ...)` | launch string contains `filesrc` and does NOT contain `audiotestsrc` |
| R1/R5 | `test_bad_audio_source_errors` | `GstEncoder::new` with a bad source | returns `Err(EncodeError::Gst(...))`, no panic |

**Acceptance test: R2 `test_default_pipeline_has_silent_audio`.** The
requirement a plausible implementation gets wrong is replacing the silent leg
unconditionally. Write the "always real audio" version first, watch R2 fail,
then implement the `Option` gate and paste both outputs.

---

## Exit Criteria

The validator runs each command from the project root it was given (`cd $ROOT`
already happens inside validate-exit-criteria.sh). **Do NOT hardcode `cd
/projects/...` in a criterion** — in worktree mode that snaps back to the main
repo which lacks the changes. Use cwd-relative commands.

- [ ] `cargo test 2>&1 | grep -qE "^test result: ok\. [1-9]"` — suite passes
- [ ] `cargo test --features gstreamer test_default_pipeline_has_silent_audio 2>&1 | grep -qE "^test result: ok\. [1-9]"` — the silent-default acceptance test passes under the gstreamer feature (R2)
- [ ] `cargo test --features gstreamer test_pipeline_with_audio_source 2>&1 | grep -qE "^test result: ok\. [1-9]"` — R3
- [ ] `cargo test --features gstreamer test_bad_audio_source_errors 2>&1 | grep -qE "^test result: ok\. [1-9]"` — R5
- [ ] `cargo clippy -- -D warnings` — clean
- [ ] `git diff --quiet -- specs/` — no spec edits (G1)

**Prose criteria**:

1. The R2 failure (always-real version) and its pass (Option-gated version)
   pasted raw, one line each, unsummed.
2. `src/encode/pipe.rs` shows `build_pipeline` and `GstEncoder::new` with the
   new `audio: Option<&str>` param.

---

## Guardrails

- **G1 — do NOT edit this spec.**
- **G2 — do NOT commit.**
- **G3 — do NOT weaken/skip/delete an existing test.**
- **G4 — no hardcoded absolute paths.** Test artefacts under `_tmp/`.
- **G5 — no `rm -rf` of run state.**
- **G6 — report raw output, never summed totals.**
- **G7 — do NOT touch `src/bin/mirror.rs`** (that's part 2) and do NOT touch
  `src/serve/`, `src/cast/`, `src/mcp/`, `src/capture/`, `src/render/`,
  `src/emu/`, `src/pipeline/`.

---

## Files to Modify (part 1 only)

| File | Change |
|------|--------|
| `src/encode/pipe.rs` | add `audio: Option<String>` param to `build_pipeline` + `GstEncoder::new`; build the audio leg conditionally (R1-R5) |
| `tests/cast_tv_tests.rs` | add R2/R3/R5 tests (acceptance test lives here) |

**Not modified**: `specs/`, `src/bin/mirror.rs`, everything else.
