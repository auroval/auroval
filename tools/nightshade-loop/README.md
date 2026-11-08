# nightshade-loop

Renders a nightshade scene: the painted master, plus a charm that is **drawn by
a shader rather than painted into the image**, on a verlet chain, baked to a
seamless loop.

```nu
python3 tools/nightshade-loop/depth.py MASTER.png DEPTH.npy
python3 tools/nightshade-loop/render.py scenes/nightshade-01.json out.png --still
python3 tools/nightshade-loop/render.py scenes/nightshade-01.json out.mp4
```

## Two scenes, two kinds of motion

Every scene gets **parallax**: a figure-eight camera path over a continuous
monocular depth map, so nothing is cut out and nothing can be left behind.
That part is generic and lives in both shaders unchanged.

What moves *within* the picture is per scene, and the two that exist are
deliberately different in kind:

| | scene-01 | scene-05 |
|---|---|---|
| moving thing | a charm on a verlet chain, **in front of** the painting | the fire inside a lantern, **a property of** the painting |
| drawn by | `nacre.frag` — analytic spheres, thin-film interference, a relit sprite | `flame.frag` — looping fbm modulating the painted panels' brightness |
| simulated | yes, 24 periods of spin-up before recording | no, there is nothing to settle |
| charm block in the JSON | required | **absent** |

A scene without a `charm` key skips the chain, the sprite and three of the
six textures. That is not a special case bolted on: scene-05's moving part is
light rather than an object, and an object simulation would have had nothing
to simulate.

## The flame, and why it is not drawn on top

`flame.frag` does **not** paint flames over the lantern. It modulates the
brightness of panels that are already painted, masked to the scene's own
emissive isolation, so the picture keeps its own lighting.

That is the README's oldest lesson applied to a second case. The charm was
first made by cutting a bead strand out of a second generated image, and it
read as a grey sliver because a cut-out carries its source's lighting into a
scene that has none of it. A procedural flame sprite would do the same.

**It loops to the pixel, and that took thought.** Advected noise is not
periodic: `fbm(p - v*t)` at `t = T` is a different field from `t = 0`, and the
cut shows as a jump every ten seconds — exactly the artefact an idle login
screen gives you all night to notice. So the field is two copies of the same
advection one period apart, crossfaded across the period with `smoothstep`.
At `w = 0` the first is unadvected; at `w = 1` the second has advected by
zero; they are the same field, so `f(0) == f(T)` exactly. `smoothstep` rather
than a linear mix because its derivative vanishes at both ends, which matches
the velocity at the seam as well as the value — a discontinuity in velocity is
visible even when the value is not.

The modulation is centred on zero, so the panels' **mean** brightness is
unchanged and only their variation is new. A flame that also raised the mean
would move the measured palette, and the palette is derived from this file.

## The emissive isolation is per scene

`load_scene` masks the frame down to the one thing allowed to bloom. What is
separable differs per scene and there is no general answer, so it is
configured:

- `"cool"` — `min(G,B) - R`. scene-01's halo is the only saturated cyan while
  the kimono is white and the skin is pink.
- `"warm"` — `R - min(G,B)`. scene-05's lantern panels are the only saturated
  orange against wet grey stone and green cedar. The same subtraction the
  other way round, and it catches 4.57% of the frame.

A **luminance** threshold is never the answer here. It has already cost this
project its near-black hair, obi and beak three separate times.

The scene JSON holds everything that is per-scene: where the master and its
depth map live, where the charm hangs, where the key light is, and the loop's
timing. Masters, depth maps and charm sprites are **assets and are not in this
repository**; the JSON points at wherever they live.

Needs a GPU and an EGL driver, but no display server and no compositor —
`moderngl` runs the shader headless. `ffmpeg` comes from the system package
manager. `depth.py` additionally needs torch and transformers and is run once
per master; rendering does not import them.

## Why any of this exists

The charm was first made by cutting a bead strand out of a second generated
image and pasting it over the first. It read as a grey sliver, because a cut-out
carries its source's lighting, grain and perspective into a scene that has none
of them. Beads are spheres, and a sphere is an equation, so they are drawn:
analytic spheres with real thin-film interference over a dark body. The skull
keeps its painted albedo and gains a normal map derived from its own alpha and
luminance, so the same halo lights both.

## What is measured, not asserted

`render.py` prints two numbers every run, and both are there because an earlier
version was wrong about them:

| | |
|---|---|
| `closure` | How far the chain is from where it started, one period later. It reads 0.0000 px because the simulation is given 24 periods to forget its initial conditions before recording, not because the motion was forced to close. |
| `pendant sway` | How far the charm actually travels. An earlier loop had ~3 px of parallax and 0.68 px of mean flow, which was invisible; this number exists so that cannot happen again unnoticed. |

## Known limits

* The depth dilation (`depth_grow`) stops the figure's silhouette smearing
  across her face, at the cost of the background within those pixels
  travelling with her. That is not occlusion. Real occlusion needs separated
  layers and something to fill the gap behind her — open item A5.
* R8's seam criterion is relative, and so gameable by grain: a noisier
  encode scores better on it. The loop here passes on the merits (the seam
  measures the same as any other frame step) but the criterion itself should
  not be trusted alone.
