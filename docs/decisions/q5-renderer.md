# Q5 — which Wayland stack and renderer?

Status: **open, narrowed by measurement.** PRD open question Q5. This is the one
remaining question that gates P1, and unlike Q1–Q4 it is a technical question
rather than a matter of taste, so most of it can be settled by measuring.

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

## Recommendation, for the maintainer to accept or reject

**`smithay-client-toolkit` for surfaces and input, plus GL sharing GStreamer's
context.** Not wgpu — not because wgpu is worse, but because R7's no-readback
budget makes external texture import mandatory, and that is the one thing wgpu
is bad at.

Decide it after the minimal GL renderer exists, not before: the only honest
reason to prefer GL is a measurement nobody has taken yet, and if the GL
renderer turns out to cost what `glimagesink` costs, the premise needs
revisiting rather than defending.

## Sub-questions this opens

- **Text rendering.** R11 wants MesloLGS at a large clock size across outputs
  at different scales. A glyph atlas is the normal answer; `cosmic-text` plus a
  GL atlas is the obvious candidate and is unevaluated.
- **One context or several.** R10 wants one surface per output at that output's
  scale. Whether a single GL context serves several surfaces, or each output
  gets its own, is unresolved and affects how the video texture is shared.
- **Lock mode.** Q6 asks whether lock mode becomes its own binary. The renderer
  choice is most of that answer, since `ext-session-lock-v1` surfaces are
  created differently but drawn identically.
