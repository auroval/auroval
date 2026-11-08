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

| pipeline | memory | format | CPU | cost of the step |
|---|---|---|---|---|
| `fakesink`, frames stay put | CUDAMemory | NV12 | **5.2%** | — |
| plain `appsink` | system | NV12 | **18.7%** | +13 to leave CUDA |
| `glupload` only | GLMemory | NV12 | **32.1%** | +13 for GL rather than system |
| `glsinkbin` | GLMemory | RGBA | **34.0%** | +2 to convert colour |

Read down that column rather than across it. The expensive thing is not GL and
it is not the colour conversion: **it is taking the frame out of CUDA memory at
all**, and doing it twice over — once to leave, once to arrive somewhere a
renderer can use. Each is worth about thirteen points.

One consequence is worth saying out loud because it is the opposite of what
"zero-copy" suggests: the GL path costs *more CPU* than downloading to system
memory. It is still the right one — a system-memory frame would then need a
`glTexImage2D` of 2560×1440 per frame from our own code, which this
measurement does not include — but the phrase has been doing some work it has
not earned.

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
- ~~**try `memory:DMABuf`**~~ — **closed, 2026-11-08.** `gst-inspect-1.0
  nvh264dec` on driver 620.70.06 advertises exactly three memory types on its
  source pad: `memory:CUDAMemory`, `memory:GLMemory` and plain `video/x-raw`.
  There is no dmabuf to import. The EGL side would have taken one —
  `EGL_EXT_image_dma_buf_import` is in this display's extensions — but the
  decoder does not offer one. Q5's "unverified for want of the hardware" is
  now "unavailable on this decoder, whatever the hardware".
- **accept it**, which is defensible for a login screen on this hardware and
  much less so for P3's lock screen, which is on display for minutes rather
  than seconds

## Two things that do not work, so nobody tries them twice

**Forcing `memory:GLMemory` on a plain `appsink` fails**, exactly as Q5 said,
and answering the context query on the bus is not enough to rescue it. The
run dies with `Internal data stream error` from qtdemux — which is what a
failed negotiation upstream looks like from the outside, and is worth
recognising because it names the wrong element.

The mechanism is worth writing down, because Q5 recorded the symptom and not
this: the decoder learns about the GL context from the **allocation query**
that travels upstream from a GL element, not from the bus. A bare `appsink`
answers no queries, so there is nothing to negotiate against. `glupload` is
therefore not an optimisation to be removed — it is the thing that makes the
negotiation possible.

**Letting the decoder produce `memory:GLMemory` itself** — it advertises it —
is the same thing by another name, and fails the same way for the same
reason.

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
- `AUROVAL_VIDEO_SYSMEM=1` takes frames in plain system memory. The
  discriminator between "GL is expensive" and "leaving CUDA is expensive", and
  the answer is the second.
- `AUROVAL_VIDEO_DIRECT=1` forces GLMemory on a bare appsink. It **fails**,
  on purpose: it is kept so the failure can be reproduced rather than
  rediscovered.

## What is not done

The frame is not drawn yet. `Video::frame()` hands back a texture id, size and
nothing else; `background.rs` does not know about it, and neither does
`view::draw`. That is the next piece, and it is deliberately after this one:
there is no point drawing a background whose transport costs six times its
budget until the budget question has an answer.
