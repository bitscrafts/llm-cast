# chromecast-tv-mirror — spec-06 part 3: E2E real-audio HLS verification

- **Project**: `/projects/chromecast-tv-mirror`
- **Priority**: part 3 (final) of spec-06 (real audio). Proves end-to-end that
  a real audio source survives into the muxed HLS stream, using in-container
  GStreamer tooling (no TV needed).
- **Status**: SPECIFIED
- **Source**: split of `06-real-audio.md`
- **Depends-On**: part 1 (encoder seam), part 2 (CLI wiring)

---

## Verified Premises

- The container has GStreamer 1.26.2 with `filesrc`, `decodebin`, `audioconvert`,
  `audioresample`, `voaacenc`, `hlssink2` — verified via `gst-inspect-1.0`.
- `src/encode/pipe.rs` (part 1) now accepts an `audio` fragment; `mirror`
  (part 2) now passes `--audio-source` through.
- A WAV file can be generated in-container with `sox`/`ffmpeg`/Python `wave`.

---

## Overview

Parts 1-2 added the seam and the CLI flag. This part proves the audio
actually lands in the HLS output: generate a WAV, run a real GStreamer
pipeline with that audio source, and verify the output TS carries an audio
stream. This is the operator-facing proof that real audio works.

---

## Requirements

### Functional

- **R1 (audio in HLS)**: with `--audio-source <wav>`, the muxed TS segments
  carry an audio stream (verified via `gst-discoverer` / `gst-inspect` on the
  output).
- **R2 (silent still works)**: with no `--audio-source`, the TS still carries
  audio (the silent AAC track) — DMR mandate preserved.
- **R3 (reproducible helper)**: a repeatable script
  (`_tmp/verify-audio.sh`) generates a WAV, runs the pipeline, and inspects
  the output — for future operator use.

### Non-Functional

- **N1**: no TV / device needed; fully in-container.
- **N2**: no new Rust dependencies.
- **N3**: does NOT touch `src/` — verification is script-based, under `_tmp/`.

---

## Architecture

```
gen WAV (sox/ffmpeg/python) → mirror --audio-source '<wav>' → HLS segments
                                                                    │
                    gst-discoverer <segment.ts> → shows audio stream ✓
```

**Key decision — script under `_tmp/`, not `src/`.** This is an operator
verification, not library code. It lives in `_tmp/` (G4) and is runnable
standalone.

---

## TDD Contract

| id | test | given | expects |
|----|------|-------|---------|
| R1 (acceptance) | `audio_hls_has_audio` | a real pipeline with a generated WAV audio source, a few frames | `gst-discoverer` on the output TS reports an audio stream |
| R2 | `audio_hls_silent_default` | pipeline without audio source | `gst-discoverer` reports an audio stream (the silent AAC) |

**Acceptance: R1.** The requirement a plausible implementation gets wrong is
proving the audio actually lands — a green unit test doesn't prove the muxed
TS carries audio. This runs the real GStreamer pipeline and inspects the real
output.

---

## Exit Criteria

The validator runs each command from the project root it was given (`cd $ROOT`
already happens inside validate-exit-criteria.sh). **Do NOT hardcode `cd
/projects/...`** — in worktree mode that snaps back to the main repo which
lacks the changes. Use cwd-relative commands.

- [ ] `test -x _tmp/audio_verify.sh && bash _tmp/audio_verify.sh 2>&1 | grep -qi "audio"` — the verification script runs and confirms an audio stream (R1)
- [ ] `test -f _tmp/audio_verify.sh` — the helper exists (R3)
- [ ] `git diff --quiet -- src/` — no src/ changes (verification only)
- [ ] `git diff --quiet -- specs/` — no spec edits (G1)

**Prose criteria**:

1. Raw `gst-discoverer` (or `gst-launch`/`gst-inspect`) output on a real
   segment, pasted, showing the audio stream is present (R1).
2. The verification script `_tmp/audio_verify.sh` is committed-ready (G4: no
   hardcoded absolute paths beyond `_tmp/`).

---

## Guardrails

- **G1 — do NOT edit this spec.**
- **G2 — do NOT commit.**
- **G4 — no hardcoded absolute paths.** Test artefacts under `_tmp/`.
- **G5 — no `rm -rf` of run state.**
- **G6 — report raw output, never summed totals.**
- **G7 — do NOT touch `src/`.** This part is verification only.

---

## Files to Modify (part 3 only)

| File | Change |
|------|--------|
| `_tmp/audio_verify.sh` | generate WAV, run the real pipeline, `gst-discoverer` the output, assert audio present |

**Not modified**: `specs/`, `src/`.
