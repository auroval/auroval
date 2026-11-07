# nightshade-loop

Renders a nightshade scene: the painted master, plus a charm that is **drawn by
a shader rather than painted into the image**, on a verlet chain, baked to a
seamless loop.

```nu
python3 tools/nightshade-loop/depth.py MASTER.png DEPTH.npy
python3 tools/nightshade-loop/render.py scenes/nightshade-01.json out.png --still
python3 tools/nightshade-loop/render.py scenes/nightshade-01.json out.mp4
```

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
