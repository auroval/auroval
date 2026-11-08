# P2 results — the video path, from the greeter's own GL context

Measured 2026-11-08 on `games-werkkamer`, kernel 7.3.3-1-cachyos, nvidia
620.70.06, against `/var/lib/auroval-spike/media/loop1440p60.h264.mp4` — the
same 12-second 1440p60 H.264 clip P0 used, so the two are comparable.

The instrument is `auroval-greet videoprobe <file> [seconds]`: it makes a
surfaceless GL context, starts the pipeline against it, and prints what was
negotiated, the delivered frame rate and this process's own CPU. No
compositor, no greetd, no root.

## What works

**The zero-copy path works from this renderer's own context**, which is the
thing Q5 established with `glimagesink` and left untested against auroval's
own EGL context.

```
caps     video/x-raw(memory:GLMemory), format=RGBA, 2560x1440, framerate=60/1
memory   GLMemory — the frame is already a texture in this context
size     2560x1440
```

**R8's gapless loop works.** `playbin3`'s `about-to-finish` hands over to the
same URI while the current stream is still playing, so the pipeline never
drains: 1788 frames in 30.0 s = **59.6/s**, across two loop boundaries on a
12-second clip. A seek on EOS would also loop and would show the seam R8
forbids — EOS means the last frame is already gone and the flush empties the
sink.

**No frames are dropped.** One 20-second run delivered exactly 720 frames
before the clip ended: 12 s × 60 fps, every one.

## What does not work: R7's budget

| pipeline | memory | format | CPU |
|---|---|---|---|
| no GL at all (P0's path) | CUDAMemory | NV12 | **5.2%** |
| `glupload` only | GLMemory | NV12 | **32.1%** |
| `glsinkbin` (adds `glcolorconvert`) | GLMemory | RGBA | **34.0%** |

R7's budget is *"no more than 5% of one core"*. Getting the frame into a
texture this renderer can draw costs **about thirty points more than that**.

Three things follow, and the third is the one that matters.

**Q5's guess about `glcolorconvert` is wrong, and now measured.** Its "what to
try next" list says *"Sampling NV12 directly with a two-texture shader removes
a full-resolution pass per frame; unmeasured."* Measured: 34.0% against 32.1%.
Under two points. The two-texture shader is not where the money is, and it
should come off the list as an optimisation — it may still be worth doing for
other reasons, but not for this.

**The cost is the CUDA→GL interop.** Per-thread CPU during a run:

```
1.34 s  multiqueue0:src     the decoder's output thread
1.24 s  gstglcontext        the GL upload thread
0.28 s  vqueue:src
0.16 s  auroval-greet       this probe's own poll loop
```

Split almost evenly between the thread that pushes the decoded frame and the
thread that uploads it. Whether that is work or the nvidia driver spinning
while it waits, this measurement cannot say — and for a budget stated in CPU
percent it does not matter.

**R7's budget was derived from a measurement that stops before the frame is
drawable.** P0 measured decode into `memory:CUDAMemory` and the PRD took 2.5%
from it. But R7 is *"play a looping video background"*, and a frame in CUDA
memory is not on anybody's screen. The budget was always going to be larger
than the number it was set from; nobody noticed because the two halves were
measured years apart in project time.

That is a question for the maintainer, not a thing to quietly fix:

- **revise R7's criterion** to cover decode *to a drawable texture*, with a
  number measured here rather than inherited from P0
- **try `memory:DMABuf`**, which Q5 lists as unverified for want of Intel or
  AMD hardware — but the nvidia driver moved from 615 to 620 today, and
  dmabuf export may now be available on this machine. An `EGLImage` import
  would avoid the interop copy entirely
- **accept it**, which is defensible for a login screen on this hardware and
  much less so for P3's lock screen, which is on display for minutes rather
  than seconds

## How to reproduce

```
nu -c 'cargo build --release'
./target/release/auroval-greet videoprobe /var/lib/auroval-spike/media/loop1440p60.h264.mp4 30
```

Two environment variables exist for attributing the cost, and both make the
greeter useless as a greeter — they are measurement scaffolding:

- `AUROVAL_VIDEO_UPLOAD_ONLY=1` replaces `glsinkbin` with a bare `glupload`,
  so frames arrive as NV12 with no colour conversion.
- `AUROVAL_VIDEO_NO_GL=1` replaces the sink with `fakesink`, which is P0's
  pipeline. It delivers no frames to the renderer by design; it exists so the
  GL cost can be attributed rather than guessed at.

## What is not done

The frame is not drawn yet. `Video::frame()` hands back a texture id, size and
nothing else; `background.rs` does not know about it, and neither does
`view::draw`. That is the next piece, and it is deliberately after this one:
there is no point drawing a background whose transport costs six times its
budget until the budget question has an answer.
