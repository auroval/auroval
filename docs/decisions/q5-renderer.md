# Q5 — which Wayland stack and renderer?

Status: **settled, 2026-11-07.** `smithay-client-toolkit` for surfaces and
input, **GL sharing GStreamer's context** for pixels. Not wgpu.

The recommendation below said to decide only after a minimal GL renderer
existed and had been measured. It now exists —
[`crates/spike-q5`](../../crates/spike-q5), results in
[`spike/q5/RESULTS.md`](../../spike/q5/RESULTS.md) — and the measurement
supports the recommendation, so the recommendation is adopted. What it also
found is a bigger thing than the renderer choice, and §"What the GL renderer
measured" records it.

The rest of this document is kept as it was written, because the reasoning
that led here is the part worth re-reading if the decision is ever reopened.

## What the P0 spike settled

`smithay-client-toolkit` was unproblematic. The spike bound `wl_compositor`,
`xdg_shell`, `wl_shm` and `wl_seat`, went fullscreen under `cage`, handled
keyboard input including key repeat, and had a configured surface 71 ms after
process start. Nothing about it argues against using it for the surfaces.

What the spike did **not** settle is how pixels get there. It used `wl_shm`,
which copies every frame through system memory, and R7's budget — set from C6 —
now forbids exactly that.

## What is settled: the zero-copy path exists

`nvh264dec` advertises three memory types on its source pad:
`memory:CUDAMemory`, `memory:GLMemory` and plain `video/x-raw`. The GL one is
the interesting one, and it is **real rather than advertised**: with no
capsfilter forcing anything, `nvh264dec ! glimagesink` negotiates
`memory:GLMemory` and plays.

That matters more than it sounds. It means NVDEC frames can land directly in an
OpenGL texture with **no auroval code doing CUDA interop at all** — GStreamer
negotiates it, given a GL context to share. The hard-looking part of Q5 turns
out to be a caps negotiation rather than a piece of engineering.

One trap worth recording: forcing it with an explicit
`video/x-raw(memory:GLMemory)` capsfilter **fails** with `not-negotiated`,
because the GL context has to be established through `GstContext` before the
caps can be fixed. Letting the elements negotiate works; helping them does not.

## What is NOT settled: whether zero-copy is actually cheaper

This is the part where the obvious conclusion does not survive measurement.

720 frames of 1440p60, as fast as possible, three runs each, stable to about
±4%:

| | wall | CPU (user+sys) | per frame |
|---|---|---|---|
| decode only, stays in CUDA memory | 1.43 s | **0.22 s** | 0.3 ms |
| via `memory:GLMemory` to `glimagesink` | 1.65 s | **1.55 s** | 2.2 ms |
| via `cudadownload` to BGRx, discarded | 1.48 s | 1.40 s | 1.9 ms |

Decode itself is nearly free — 0.3 ms of CPU per 1440p frame, which is where
C6's 2.5%-of-a-core figure comes from. But **the GL path and the readback path
cost about the same**, and both cost roughly six times what decoding does.

Two reasons not to read that table as "zero-copy is pointless":

- `glimagesink` is a general-purpose sink. It manages its own window, runs its
  own NV12-to-RGB shader and commits to Wayland itself. Its cost is not a lower
  bound for what auroval would spend, and most of that 2.2 ms is presentation
  rather than interop.
- The readback row **discards** its frames rather than presenting them, so it
  is not a fair comparison in the other direction either. A real readback
  renderer would still have to upload and draw after downloading.

So the measurement settles the *capability* question and not the *cost*
question. An earlier reading of a single faster run suggested the interop was
nearly free; repeated runs do not support that, and the claim is withdrawn.

**What would settle it:** a minimal GL renderer in auroval, drawing the video
texture and one text label, measured at realtime 1440p against the spike's
`wl_shm` path. That is P1 work and perhaps a day of it.

## What the GL renderer measured

Built and measured on 2026-11-07. Both paths present the same 2560×1440 60 fps
clip to the same fullscreen surface, paced by Wayland frame callbacks, each
drawing one line of 44 px text. Every run held 60 fps with no dropped frames.

| pair | GL | shm | shm ÷ GL |
|---|---:|---:|---:|
| quiet batch, means | 44.3% | 70.2% | 1.58 |
| busy 1–6 | 55–60% | 88–107% | 1.50–1.93 |

So the GL path is **1.5–1.9× cheaper**, median about 1.65, and that ratio holds
across a 2× shift in the machine's own load. The earlier `glimagesink`
measurement could not show this because a general-purpose sink's own
presentation cost swamped the difference. Writing the renderer was the only way
to separate them, which is exactly why the brief declined to decide without it.

**The absolute numbers are not usable, and an earlier draft of this section
used them anyway.** It read 44% and 70% off the quiet batch and proposed 50% of
a core as R7's ceiling. The busy batch then put the same GL path at 60%. A
budget written from that would have encoded the maintainer's desktop rather
than the greeter, so it is withdrawn; R7's whole-path ceiling waits on a `cage`
measurement.

**What does survive, because the gap is far too large to be load:** P0 measured
decode alone at **2.5% of one core**. Presentation costs more than an order of
magnitude beyond that, in every batch. The decoder is not where a greeter's CPU
goes, and R7's budget — written as a decode budget because decode was the thing
P0 could measure — constrains only the cheap half. That is a finding about the
requirement, not about the renderer, and it is carried back into the PRD.

The one limit that matters: the spike ran inside a live Hyprland session rather
than under `cage`. That is cheap to fix — `spike/p0/` already has the scripts —
and it is the next thing to do.

## The constraint that actually decides this

Portability, from C6. Three memory types must all work:

| platform | fast path | status |
|---|---|---|
| NVIDIA / `nvcodec` | `memory:GLMemory` | **measured working** |
| Intel / AMD / `va` | `memory:DMABuf` | unverified; needs that hardware |
| anything, R9 fallback | system memory | trivially available |

This is what makes the renderer choice awkward, and it is an argument against
the otherwise obvious option.

### wgpu

The ergonomic choice, and the one the PRD originally floated. The problem is
that **wgpu has no stable, safe way to import an externally created texture** —
neither a GL texture from `glupload` nor a dmabuf. Doing it means dropping to
`wgpu-hal` and unsafe platform code, which is more interop engineering than the
GL route avoided, and it has to be written twice (GL on NVIDIA, dmabuf
elsewhere).

wgpu would be the right answer if frames arrived in system memory. R7's budget
is what rules that out.

### GL, sharing GStreamer's context

`gstreamer-gl` exposes the `GstGLContext` and `GstGLDisplay` that the pipeline
negotiates against, so auroval can create the context, hand it to GStreamer,
and receive textures it can already draw. The dmabuf path on Intel and AMD
imports through `EGL_EXT_image_dma_buf_import`, which this machine's EGL
already advertises — `cage`'s own startup log lists it.

Cost: GL is the older, less pleasant API, and all text rendering becomes
auroval's problem (a glyph atlas, not `fontdue` blitting into a CPU buffer as
the spike did).

### hyprtoolkit

Did not exist when Q5 was written; it is now a package on this machine and a
`hyprlang` consumer. Worth a look for R11's geometry, but it pulls the project
back towards C++ and towards tracking an upstream, both of which the PRD already
rejected when it ruled out forking hyprlock. **Unexamined**, and noted only so
the question is not reopened later as if it had been missed.

## Recommendation, accepted 2026-11-07

**`smithay-client-toolkit` for surfaces and input, plus GL sharing GStreamer's
context.** Not wgpu — not because wgpu is worse, but because R7's no-readback
budget makes external texture import mandatory, and that is the one thing wgpu
is bad at.

Decide it after the minimal GL renderer exists, not before: the only honest
reason to prefer GL is a measurement nobody has taken yet, and if the GL
renderer turns out to cost what `glimagesink` costs, the premise needs
revisiting rather than defending.

> **It did not.** The renderer was built, it negotiates `memory:GLMemory` with
> `nvh264dec` for real, and it costs 1.6× less than the readback path at
> 1440p60. The premise held and the recommendation is adopted. Had it come out
> level, portability alone would still have pointed the same way — but it
> would have been a much weaker decision, and worth saying so.

## Sub-questions this opens

Updated after the spike. The first of these is now the most valuable thing to
measure next, because presentation rather than decode turns out to be where
the CPU goes.

- **Skip `glcolorconvert`.** The spike converts to RGBA so one texture can be
  sampled. Sampling NV12 directly with a two-texture shader removes a
  full-resolution pass from every frame. Unmeasured, and the obvious next step.
- **Measure under `cage`, not in a live session.** Both figures are inflated by
  the desktop around them.

- **Text rendering.** R11 wants MesloLGS at a large clock size across outputs
  at different scales. A glyph atlas is the normal answer; `cosmic-text` plus a
  GL atlas is the obvious candidate and is unevaluated. The spike shows the
  *cost* is not the issue — one line of 44 px text was below run-to-run
  variance on both paths — so this is a question of engineering effort, not of
  budget.
- **One context or several.** R10 wants one surface per output at that output's
  scale. Whether a single GL context serves several surfaces, or each output
  gets its own, is unresolved and affects how the video texture is shared.
- **Lock mode.** Q6 asks whether lock mode becomes its own binary. The renderer
  choice is most of that answer, since `ext-session-lock-v1` surfaces are
  created differently but drawn identically.
