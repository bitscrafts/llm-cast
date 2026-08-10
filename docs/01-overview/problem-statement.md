# Problem statement

## Goal
Display the content of a terminal multiplexer session on a TV, using a hardware
**Google Chromecast** attached to that TV. The terminal tool is an
_external/`outer` "herd" multiplexer_ (client/server; pending confirmation of
exact identity — likely a tmux-like tool with remote/attach support).

## Constraints & open questions
- **Chromecast is a headless Cast device** — there is no local OS you control;
  it only runs Google Cast (Cast v2) receiver apps. You cannot install arbitrary
  software on it.
- Need to confirm: Chromecast model/gen (determines codec support: H.264/AV1,
  and "gen2+" vs older), network topology (same subnet as the muxer host?),
  and whether the muxer has a web/HTTP exposure or only a terminal.
- Latency budget: for a terminal feed, ideally < 1s end-to-end (interactive).
- Chromecast has no display buffer you can screen-cap from the host; you must
  push a stream or a page TO the device.

## Deliverable (research phase)
A concrete, buildable architecture: how to get pixels/text from the muxer host
to the TV, with the fewest moving parts, lowest latency, and no Google-registered
receiver-application development if avoidable.
