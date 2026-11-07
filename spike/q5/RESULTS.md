# Q5 spike — what a real GL renderer costs against `wl_shm`

Run on 2026-11-07. The Q5 brief refused to decide without this measurement:
*"a minimal GL renderer in auroval, drawing the video texture and one text
label, measured at realtime 1440p against the spike's `wl_shm` path."*

Source: [`crates/spike-q5`](../../crates/spike-q5). Throwaway, like `spike-p0`.

## How it was measured

| | |
|---|---|
| Clip | `loop1440p60.h264.mp4` — 2560×1440, 60 fps, 720 frames, 12.0 s |
| Surface | fullscreen, 2560×1440, reported by the tool rather than assumed |
| Window | 10 s, inside the clip's 12 s, so no part of it is idle after EOS |
| Measured from | the first presented frame, so pipeline construction is excluded |
| CPU | `/proc/self/stat` utime+stime, which counts every thread GStreamer starts |
| Pacing | Wayland frame callbacks in both modes; `appsink sync=true` in both |
| Label | one line of MesloLGS at 44 px, drawn every frame in both modes |

Both modes present the same clip to the same surface at the same cadence. The
decode and upload paths are what differs.

**The GL path was verified, not assumed.** It prints its negotiated caps and
its element list on the first frame:

```
negotiated  video/x-raw(memory:GLMemory), format=(string)RGBA,
            width=(int)2560, height=(int)1440, framerate=(fraction)60/1,
            texture-target=(string)2D
elements    sink glcolorconvertelement0 gluploadelement0 decodebin3-0
            nvh264dec0 parsebin0 capsfilter0 h264parse0 qtdemux0 typefind
            multiqueue0 filesrc0
```

`memory:GLMemory` and `nvh264dec`. A GL path that had quietly fallen back to
system memory would still have run, and the number would then be of something
else entirely.

## The numbers

Every run held 60.0–60.3 fps with no dropped frames, so these are costs at the
same delivered result.

**The absolute figures moved while this was being measured**, and that has to
come first because it changes what the table is good for. An early quiet-machine
batch gave GL 33–52% and shm 69–71%. A later batch, with more running on the
desktop, gave GL 55–60% and shm 88–107% — the *same binary, same clip, same
surface*. A live Hyprland session is simply not a measurement environment.

What survives that is the **ratio**, measured as back-to-back pairs:

| pair | GL | shm | shm ÷ GL |
|---|---:|---:|---:|
| quiet batch, means | 44.3% | 70.2% | 1.58 |
| busy 1 | 60.4% | 102.4% | 1.70 |
| busy 2 | 55.2% | 93.0% | 1.68 |
| busy 3 | 55.2% | 106.7% | 1.93 |
| busy 4 | 56.7% | 87.8% | 1.55 |
| busy 5 | 58.7% | 88.3% | 1.50 |
| busy 6 | 57.1% | 90.9% | 1.59 |

**GL costs 1.5–1.9× less than the readback path**, median about 1.65, stable
across a 2× shift in the machine's own load. That is the result.

### What the absolute numbers are NOT good for

They cannot set a budget. An earlier draft of this document proposed 50% of one
core as R7's ceiling, read straight off the quiet batch; the busy batch then put
the same GL path at 60%. Writing a requirement against a number that moves with
what else is on screen would have encoded the desktop, not the greeter. **R7's
whole-path budget needs a `cage` measurement**, in the context the greeter
actually runs in, and does not have one yet.

What the numbers do establish, because the gap is far too large to be load:
P0 measured decode alone at **2.5% of one core**, and presentation costs more
than an order of magnitude beyond it in every batch. The decoder is not where a
greeter's CPU goes.

### The text label is below the noise

With the label removed, GL measured 40.0% and 40.2%; shm measured 73.5% and
94.7% — against 69–71% *with* the label in the same batch. One line of 44 px
text is not separable from run-to-run variance on either path, and the "cost"
of removing it came out negative on shm, which is how you know you are reading
noise. It does not decide anything, which is itself useful:
the glyph question is a question of engineering effort, not of cost.

## What this does not establish

* **Measured inside a running Hyprland session**, with the maintainer's own
  desktop and other clients on screen. P0 measured under `cage` on a TTY,
  which is the greeter's real context. This is the single biggest weakness of
  the spike: the absolute figures moved by 2× with load, so only the ratio can
  be relied on, and a budget cannot be set from this at all. **A `cage` run is
  the next thing to do**, and it is cheap — `spike/p0/` already has the
  scripts for driving one.
* **`memory:DMABuf` on Intel or AMD remains unverified.** There is no such
  hardware here. This is the same gap tenet 2 records, and it is unchanged.
* **`glcolorconvert` is in the measured path.** The pipeline converts to RGBA
  so the shader can sample one texture. Sampling NV12 directly with a
  two-texture shader would remove a full-resolution pass, and is the obvious
  next measurement rather than a claim.
* **Nothing here was run as greetd's own user.** P0 established that decode
  works there; this spike ran as the maintainer.

## Reproducing

```bash
cargo build -p spike-q5 --release
./target/release/spike-q5 gl  /var/lib/auroval-spike/media/loop1440p60.h264.mp4 10
./target/release/spike-q5 shm /var/lib/auroval-spike/media/loop1440p60.h264.mp4 10
AUROVAL_NO_TEXT=1 ./target/release/spike-q5 gl <clip> 10   # label removed
```

The clip must be longer than the measurement window, or the run spends its
tail idle after EOS and the percentage comes out low. An earlier 15 s run
against this 12 s clip reported 44% where the live figure was 70%.
