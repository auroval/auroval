# P1/D — what a still background costs, and therefore when it is decoded

Run on 2026-11-07, for issue #34. The brief for piece D named the question
rather than the answer: *"the interesting question is **when** the decode
happens. R16's budget is first frame after launch, and a 4K PNG decode is not
free. Decoding on a thread and showing the solid colour until it arrives may be
the better answer; measuring it is part of this."*

So it was measured first and the design followed. The harness is not a throwaway
crate this time — it is an `#[ignore]`d test next to the code it measures, so
the claim can be rechecked on other hardware without reviving a spike:

```nu
cargo test --release -p greet -- --ignored --nocapture measure_decode_cost
```

## How it was measured

| | |
|---|---|
| Machine | 11th Gen Intel Core i9-11900KF @ 3.50 GHz, 16 threads |
| Build | `--release` (`opt-level = 3`), rustc 1.94.1 |
| Decoder | `image` 0.25.10, `default-features = false`, features `png` and `jpeg` |
| Measured | `decode()` whole: the R15 path check, `canonicalize`, open, decode, and `to_rgba8` |
| Rounds | 5 per fixture; best and mean both reported |
| Page cache | **warm.** See the caveat below — this is the limit of the measurement |

Fixtures are encoded by the test rather than checked in, so they cannot rot.
Two kinds, deliberately, because PNG decode cost depends on the content and one
fixture would have answered the wrong question:

- **gradient** — a smooth ramp. Deflates to almost nothing (168 KiB for 4K) but
  still inflates to the full 32 MB and still pays per-row unfiltering.
- **noise** — xorshift32 bytes. Incompressible, so the encoder stores it raw:
  32 MB on disk, no filtering, nothing for inflate to do.

## The numbers

| Fixture | File | Decoded | Best | Mean |
|---|---:|---:|---:|---:|
| 1080p gradient PNG | 44 KiB | 8.1 MB | 7.4 ms | 7.5 ms |
| 1440p gradient PNG | 77 KiB | 14.4 MB | 13.4 ms | 13.8 ms |
| **4K gradient PNG** | 168 KiB | 32.4 MB | **33.5 ms** | 33.8 ms |
| 4K gradient JPEG | 634 KiB | 32.4 MB | 26.5 ms | 28.6 ms |
| 4K noise PNG | 32 404 KiB | 32.4 MB | 11.4 ms | 12.8 ms |
| 1440p noise PNG | 14 402 KiB | 14.4 MB | 4.0 ms | 4.4 ms |

### What the two kinds of PNG say together

The noise fixture is **190× the file size of the gradient at the same
resolution and decodes three times faster**, which is the useful result. PNG
decode here is not paid per compressed byte; it is paid per *output* byte, in
unfiltering and in writing 32 MB. The noise case skips the filtering entirely
(stored, unfiltered) and lands at 11 ms, which is close to the floor for moving
32 MB about. The gradient's extra ~22 ms is inflate plus unfiltering.

So a real 4K photograph — compressed, and filtered row by row — sits at or a
little above the 33 ms gradient figure, not at the 11 ms noise one. **Call it
30–40 ms for a 4K still on this machine.** JPEG is slightly cheaper than PNG at
the same resolution, which is the opposite of the intuition that a bigger file
costs more.

## What that means against R16

R16's budget is 500 ms from process start to first presented frame, and P0
measured 243 ms of it already spent. 30–40 ms is about 7% of the budget and
would, on its own, fit synchronously. **The measurement does not by itself
justify a thread.** Three things do:

1. **The read is not in the number.** Every round above hit a warm page cache.
   The greeter runs seconds after boot, when the cache is cold and the disk is
   contended by everything else starting — and `decode()` pays the read as well
   as the decode. That cost is unmeasured here and is plausibly the larger half.
2. **Overlapping is free.** The main thread's first 243 ms is spent waiting on a
   Wayland surface, an EGL context and font rasterisation, none of which needs
   the CPU the decoder wants. On any multi-core machine the decode disappears
   into that wait entirely.
3. **It removes a whole class of regression.** With the decode on the main
   thread, a background that is large, on a slow disk, or on a network mount
   delays the *password prompt*. With it on a thread, the worst case is that
   the login screen shows its solid colour for a few frames and the still
   appears underneath a prompt that was already usable. That is R9's own
   fallback doing ordinary duty rather than an error path.

Point 3 is the one that decided it. The cost of being wrong is asymmetric: a
thread that turns out to be unnecessary wastes about thirty lines, while a
synchronous decode that turns out to be slow sits in front of a login.

**Decision: `Background::start` spawns a decoder thread and returns at once;
the solid colour is drawn until the still lands, and the first frame never
waits on either.** The upload happens on the first frame after the pixels
arrive, because `tex_image_2d` has to be on the thread holding the GL context.

### The caveat, stated rather than buried

**Cold-cache read is not measured.** Doing it properly means dropping the page
cache between rounds, which needs root:

```nu
sync; echo 3 | sudo tee /proc/sys/vm/drop_caches
```

`AGENTS.md` forbids this agent from running `sudo` at all, so the command is
left here for the maintainer rather than guessed at. It would only strengthen
the conclusion: a cold read can only add to the 30–40 ms, never subtract. The
threaded design is correct under both the measured and the unmeasured number,
which is why the piece was not held up for it.

**The texture upload is not measured either.** A 32 MB `tex_image_2d` is not
free, and it is on the main thread by necessity. It happens exactly once per
run, and Q5 already established that presentation rather than decode is where a
greeter's CPU goes — [`spike/q5/RESULTS.md`](../q5/RESULTS.md). Worth a number
once piece A can present a real frame; it cannot change the decision above,
because moving the upload off the main thread is not possible with one context.

## Fit criteria this file is the receipt for

Issue #34's checklist item *"decoding happens once, not per frame"* is covered by
the design rather than by a number: `Background` holds a `Ready { tex, w, h }`
after the single upload, and `draw` is then one `draw_image` call. The test
`a_configured_background_is_decoded_once_off_the_main_thread` asserts that the
channel delivers exactly one result and is then disconnected.
