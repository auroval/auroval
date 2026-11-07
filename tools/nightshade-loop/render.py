"""Render a nightshade scene: the painted master, plus a shader-drawn charm.

Run it with `--still` for one frame to judge, or `--loop` to bake the video.

Why the charm is drawn rather than painted
------------------------------------------
The first attempt cut a bead strand out of a second generated image and pasted
it over the first. It read as a grey sliver, because a cut-out carries its
source's lighting, its grain and its perspective into a scene that has none of
them. A bead is a sphere, and a sphere is an equation --- it is one of the few
things a shader does strictly better than a painting, so it is drawn here:

  * **Beads** are analytic spheres with real thin-film interference over a dark
    body. The colour comes from the optical path `2*n*d*cos(theta)` through an
    aragonite film a few hundred nanometres thick, which is why it travels
    across the bead as the angle changes instead of sitting on it.
  * **The skull** keeps its painted albedo and gains a normal map built from
    its own alpha (a dome, so it is an object and not a sticker) and its own
    luminance (the carved detail). It is then lit by the same halo as the beads.
  * **The bloom** is computed from a cyan-masked copy of the scene, so only the
    disc spills. Blooming the whole frame also blew out her kimono and her
    face: 14.3% of pixels clipped against 0.066% in the painting itself, and
    10.4% of the frame was clipped on her rather than on the disc.

Why the loop closes
-------------------
Two things have to be true at once and they pull against each other: the charm
must obey physics, because a bead strand moving on a sine is read instantly as
fake, and the video must loop seamlessly, which a free simulation never does.

The way out is to stop trying to make the simulation *return* and instead let
it *forget*. A damped chain under a periodic drive has a steady state whose
period is the driving period, and the transient from the initial conditions
decays. So: drive at a harmonic of the loop, run long enough for the transient
to die, and only then record. The closure is measured and printed, not asserted.

The drive sits near the chain's own natural period on purpose. Off resonance a
chain merely follows its anchor; near it, it swings several times further than
the drive does, which is the visible motion tenet 2 asks for.
"""

import argparse
import json
import subprocess
from pathlib import Path

import cv2
import moderngl
import numpy as np

HERE = Path(__file__).resolve().parent
VERTEX = """#version 330
in vec2 in_pos; out vec2 uv;
void main(){ uv = in_pos*0.5+0.5; gl_Position = vec4(in_pos, 0.0, 1.0); }
"""


# --------------------------------------------------------------------- assets
def load_scene(cfg: dict) -> tuple[np.ndarray, np.ndarray, np.ndarray]:
    """Return (scene RGB 0..1, depth 0..1, emissive copy) for the master."""
    bgr = cv2.imread(cfg["master"], cv2.IMREAD_UNCHANGED)
    if bgr is None:
        raise SystemExit(f"cannot read master {cfg['master']}")
    scale = 65535.0 if bgr.dtype == np.uint16 else 255.0
    scene = cv2.cvtColor(bgr, cv2.COLOR_BGR2RGB).astype(np.float32) / scale

    depth = np.load(cfg["depth"]).astype(np.float32)
    # A one-step displacement reads the depth at the DESTINATION, so next to a
    # silhouette a foreground pixel takes its colour from behind the figure and
    # the edge smears across her face. Growing the near depth outward by more
    # than the largest displacement makes those samples land on the figure
    # instead. The honest cost: the background within `grow` pixels of her edge
    # now travels with her. Against a glowing disc that is far less visible
    # than a ghost of her jawline, but it is not occlusion --- real occlusion
    # needs separated layers and something to fill the gap behind her.
    grow = int(cfg.get("depth_grow", 25))
    depth = cv2.dilate(depth, cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (grow * 2 + 1,) * 2))
    depth = cv2.GaussianBlur(depth, (0, 0), 10.0)

    # The disc alone, for the bloom. Cyan is the separable thing here: the halo
    # is the only saturated cyan in the frame while her kimono is white and her
    # skin is pink, so min(G,B)-R isolates it without cutting anything out. A
    # luminance threshold would not: it has already cost this project its
    # near-black hair, obi and beak three separate times.
    cy = np.clip(np.minimum(scene[..., 1], scene[..., 2]) - scene[..., 0], 0.0, 1.0)
    emissive = scene * np.clip((cy - 0.05) / 0.20, 0.0, 1.0)[..., None]
    return scene, depth, emissive


def load_charm(cfg: dict) -> dict:
    """Load the pendant sprite and derive its normal map and hanging point."""
    npz = np.load(cfg["sprite"])
    rgb = npz["img"].astype(np.float32) / (65535.0 if npz["img"].dtype == np.uint16 else 255.0)
    alpha = npz["alpha"].astype(np.float32)

    # Trim to the alpha's own bounding box, so placement is about where the
    # bone is rather than where the crop happened to fall. The threshold is
    # 0.35, not ~0: a soft matting edge reaches almost the whole frame.
    ys, xs = np.nonzero(alpha > 0.35)
    rgb = rgb[ys.min() : ys.max() + 1, xs.min() : xs.max() + 1]
    alpha = alpha[ys.min() : ys.max() + 1, xs.min() : xs.max() + 1]

    h = int(cfg["height"])
    w = round(alpha.shape[1] * h / alpha.shape[0])
    rgb = cv2.resize(rgb, (w, h), interpolation=cv2.INTER_AREA)
    alpha = cv2.resize(alpha, (w, h), interpolation=cv2.INTER_AREA)

    dist = cv2.distanceTransform((alpha > 0.5).astype(np.uint8), cv2.DIST_L2, 5)
    dome = np.sqrt(np.clip(dist / max(float(dist.max()), 1e-6), 0.0, 1.0))
    lum = rgb @ np.array([0.2126, 0.7152, 0.0722], np.float32)
    detail = lum - cv2.GaussianBlur(lum, (0, 0), 3.0)
    height = cv2.GaussianBlur(dome + detail * 0.30, (0, 0), 2.0)
    gx = cv2.Sobel(height, cv2.CV_32F, 1, 0, ksize=3)
    gy = cv2.Sobel(height, cv2.CV_32F, 0, 1, ksize=3)
    nrm = np.dstack([-gx * 18.0, gy * 18.0, np.ones_like(gx)])
    nrm /= np.linalg.norm(nrm, axis=2, keepdims=True)

    # Hang it from the crown, not from the centre of its box. The beak sticks a
    # long way out to one side, which drags the box's centre with it and leaves
    # the cranium --- the part a pendant actually hangs by --- floating beside
    # the chain instead of under it.
    solid = alpha > 0.35
    rows = np.nonzero(solid.any(axis=1))[0]
    band = solid[rows[0] : rows[0] + max(3, h // 10)]
    crown = np.array([float(np.nonzero(band)[1].mean()), float(rows[0])])
    return {"rgb": rgb, "alpha": alpha, "nrm": nrm, "crown": crown, "w": w, "h": h}


# -------------------------------------------------------------------- physics
class Chain:
    """A verlet strand: bead 0 is pinned, the last point is the pendant's crown.

    The pin does NOT move. Driving the strand by sliding its anchor made the
    top bead slide against the painted hairpin it hangs from, which reads as a
    bead attached to nothing. A sideways acceleration --- air --- drives the
    same resonance while leaving bead 0 welded to the pin.
    """

    def __init__(self, cfg: dict, loop: dict):
        self.anchor = np.asarray(cfg["anchor"], np.float64)
        self.n = int(cfg["beads"])
        seg = float(cfg["bead_radius"]) * 1.62
        self.links = [seg] * (self.n - 1) + [float(cfg["bead_radius"]) * 0.5]
        self.g = float(loop["gravity"])
        self.wind = float(loop["wind"])
        self.harmonic = int(loop["drive_harmonic"])
        self.period = float(loop["seconds"])
        self.damping = float(loop["damping"])
        self.pts = np.array(
            [self.anchor + [0.0, sum(self.links[:i])] for i in range(len(self.links) + 1)]
        )
        self.prev = self.pts.copy()

    def step(self, t: float, dt: float) -> None:
        vel = (self.pts - self.prev) * self.damping
        self.prev = self.pts.copy()
        accel = np.array([self.wind * np.sin(2 * np.pi * self.harmonic * t / self.period), self.g])
        self.pts = self.pts + vel + accel * dt * dt
        self.pts[0] = self.anchor
        for _ in range(14):
            for i, length in enumerate(self.links):
                delta = self.pts[i + 1] - self.pts[i]
                dist = max(float(np.linalg.norm(delta)), 1e-9)
                corr = delta * (1.0 - length / dist)
                if i == 0:
                    self.pts[i + 1] -= corr
                else:
                    self.pts[i] += corr * 0.5
                    self.pts[i + 1] -= corr * 0.5
            self.pts[0] = self.anchor


def settle(chain: Chain, frames: int, sub: int, periods: int) -> tuple[list[np.ndarray], float]:
    """Discard the transient, then record exactly one period. Returns (states, closure)."""
    dt = chain.period / (frames * sub)
    t = 0.0
    for _ in range(periods * frames * sub):
        chain.step(t, dt)
        t += dt
    states = []
    for _ in range(frames):
        states.append(chain.pts.copy())
        for _ in range(sub):
            chain.step(t, dt)
            t += dt
    closure = float(np.linalg.norm(chain.pts - states[0], axis=1).max())
    return states, closure


# --------------------------------------------------------------------- render
class Renderer:
    def __init__(self, cfg: dict):
        self.scene, self.depth, self.emissive = load_scene(cfg)
        self.charm = load_charm(cfg["charm"])
        self.h, self.w = self.scene.shape[:2]
        self.cfg = cfg

        self.ctx = moderngl.create_context(standalone=True, backend="egl")
        self.prog = self.ctx.program(
            vertex_shader=VERTEX, fragment_shader=(HERE / "nacre.frag").read_text()
        )
        quad = self.ctx.buffer(np.array([-1, -1, 1, -1, -1, 1, 1, 1], "f4"))
        self.vao = self.ctx.vertex_array(self.prog, [(quad, "2f", "in_pos")])

        c = self.charm
        textures = [
            self.ctx.texture((self.w, self.h), 3, self.scene[::-1].copy().tobytes(), dtype="f4"),
            self.ctx.texture((c["w"], c["h"]), 3, c["rgb"][::-1].copy().tobytes(), dtype="f4"),
            self.ctx.texture((c["w"], c["h"]), 1, c["alpha"][::-1].copy().tobytes(), dtype="f4"),
            self.ctx.texture(
                (c["w"], c["h"]), 3, c["nrm"].astype("f4")[::-1].copy().tobytes(), dtype="f4"
            ),
            self.ctx.texture((self.w, self.h), 1, self.depth[::-1].copy().tobytes(), dtype="f4"),
            self.ctx.texture((self.w, self.h), 3, self.emissive[::-1].copy().tobytes(), dtype="f4"),
        ]
        for i, tex in enumerate(textures):
            tex.filter = (moderngl.LINEAR, moderngl.LINEAR)
            tex.repeat_x = tex.repeat_y = False
            tex.use(i)
        self.textures = textures

        ch = cfg["charm"]
        for key, value in (
            ("scene", 0),
            ("alb", 1),
            ("salpha", 2),
            ("snrm", 3),
            ("dep", 4),
            ("emis", 5),
            ("res", (float(self.w), float(self.h))),
            ("halo", tuple(float(v) for v in ch["halo"])),
            ("brad", float(ch["bead_radius"])),
            ("nbead", int(ch["beads"])),
            ("skullHalf", (c["w"] * 0.5, c["h"] * 0.5)),
            ("grain", float(cfg.get("grain", 0.012))),
        ):
            self.set(key, value)

    def set(self, key, value):
        # Unused uniforms are optimised out of the linked program, so setting
        # one must not be an error.
        if key in self.prog:
            self.prog[key] = value

    def place(self, beads: np.ndarray, tip: np.ndarray, tilt: float) -> None:
        c = self.charm
        cos_t, sin_t = np.cos(tilt), np.sin(tilt)
        off = c["crown"] - np.array([c["w"] * 0.5, c["h"] * 0.5])
        # The sprite is drawn rotated about its own centre, so the crown's
        # offset from that centre has to be rotated the same way.
        off = np.array([off[0] * cos_t - off[1] * sin_t, off[0] * sin_t + off[1] * cos_t])
        pad = np.zeros((24, 2), "f4")
        pad[: beads.shape[0]] = beads
        self.prog["beads"].write(pad.tobytes())
        self.set("skullC", tuple(float(v) for v in (tip - off)))
        self.set("tilt", float(tilt))

    def draw(self, fbo) -> np.ndarray:
        fbo.use()
        self.ctx.clear()
        self.vao.render(moderngl.TRIANGLE_STRIP)
        buf = np.frombuffer(fbo.read(components=3, dtype="f4"), "f4")
        # GL's origin is bottom-left and the textures went up flipped, so the
        # read-back comes out upside down.
        return buf.reshape(self.h, self.w, 3)[::-1]


def rest_pose(cfg: dict, charm: dict, tilt: float) -> tuple[np.ndarray, np.ndarray]:
    """Where the beads hang with no motion. The first bead sits ON the pin."""
    anchor = np.asarray(cfg["anchor"], np.float64)
    seg = float(cfg["bead_radius"]) * 1.62
    beads = np.array(
        [
            anchor + [np.sin(tilt) * seg * i, np.cos(tilt) * seg * i]
            for i in range(int(cfg["beads"]))
        ]
    )
    tip = beads[-1] + np.array([np.sin(tilt), np.cos(tilt)]) * float(cfg["bead_radius"]) * 0.5
    return beads, tip


def write_png(path: str, rgb: np.ndarray, sixteen: bool = True) -> None:
    rgb = np.clip(rgb, 0.0, 1.0)
    data = (rgb * 65535).astype(np.uint16) if sixteen else (rgb * 255).astype(np.uint8)
    cv2.imwrite(path, cv2.cvtColor(data, cv2.COLOR_RGB2BGR))


def main() -> None:
    ap = argparse.ArgumentParser(description="Render a nightshade scene still or loop.")
    ap.add_argument("scene", help="scene JSON (see scenes/ for the shape)")
    ap.add_argument("out", help="output .png for --still, .mp4 for --loop")
    ap.add_argument("--still", action="store_true", help="one frame, for judging")
    ap.add_argument("--tilt", type=float, default=0.10, help="chain angle for --still, radians")
    ap.add_argument(
        "--spinup", type=int, default=24, help="loop periods discarded before recording"
    )
    args = ap.parse_args()

    cfg = json.loads(Path(args.scene).read_text())
    for key in ("master", "depth"):
        cfg[key] = str((Path(args.scene).parent / cfg[key]).resolve())
    cfg["charm"]["sprite"] = str((Path(args.scene).parent / cfg["charm"]["sprite"]).resolve())

    r = Renderer(cfg)
    ch = cfg["charm"]

    if args.still:
        beads, tip = rest_pose(ch, r.charm, args.tilt)
        r.place(beads, tip, args.tilt)
        r.set("shift", (0.0, 0.0))
        r.set("glow", float(cfg["loop"]["glow_mean"]))
        r.set("time", 0.0)
        fbo = r.ctx.framebuffer(color_attachments=[r.ctx.texture((r.w, r.h), 3, dtype="f4")])
        write_png(args.out, r.draw(fbo))
        print(f"still -> {args.out}")
        return

    loop = cfg["loop"]
    fps, seconds = int(loop["fps"]), float(loop["seconds"])
    frames, sub = int(seconds * fps), int(loop.get("substeps", 8))
    chain = Chain(ch, loop)
    states, closure = settle(chain, frames, sub, args.spinup)
    sway = float(np.ptp([s[-1][0] for s in states]))
    print(f"closure {closure:.4f} px over one period   pendant sway {sway:.1f} px")

    # The charm hangs in front of the painting, so it takes the camera's
    # parallax too. Two things have to be right: the depth is read AT the pin
    # rather than guessed, and the sign is the shader's inverse, because the
    # shader moves the SAMPLE and therefore moves the image the other way.
    anchor = np.asarray(ch["anchor"], int)
    d_anchor = float(r.depth[anchor[1], anchor[0]])

    fbo = r.ctx.framebuffer(color_attachments=[r.ctx.texture((r.w, r.h), 3, dtype="f1")])
    ff = subprocess.Popen(
        [
            "ffmpeg",
            "-nostdin",
            "-y",
            "-loglevel",
            "error",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-s",
            f"{r.w}x{r.h}",
            "-r",
            str(fps),
            "-i",
            "-",
            "-c:v",
            "libx265",
            "-preset",
            "slow",
            "-crf",
            str(loop.get("crf", 16)),
            "-pix_fmt",
            "yuv420p",
            "-tag:v",
            "hvc1",
            args.out,
        ],
        stdin=subprocess.PIPE,
    )
    n = int(ch["beads"])
    for f in range(frames):
        phase = f / frames
        w = 2 * np.pi * phase
        # A figure-eight camera path: x at n=1, y at n=2. Both are harmonics of
        # the loop, so value AND velocity return at the boundary. The first
        # attempt used ~3px and measured 0.68px of mean flow, which was simply
        # invisible; the 2.5D literature puts artefacts above ~80px, so there
        # is room between "nothing" and "broken".
        shift = np.array([loop["pan_x"] * np.sin(w), loop["pan_y"] * np.sin(2 * w)])
        par = -shift * np.array([r.w, r.h]) * (d_anchor - 0.35) * np.array([1.0, -1.0])

        s = states[f]
        seg = s[n] - s[n - 1]
        r.place(s[0:n] + par, s[n] + par, float(np.arctan2(seg[0], max(seg[1], 1e-6))))
        r.set("shift", tuple(float(v) for v in shift))
        r.set("glow", float(loop["glow_mean"] + loop["glow_swing"] * np.sin(2 * w)))
        r.set("time", phase)
        fbo.use()
        r.ctx.clear()
        r.vao.render(moderngl.TRIANGLE_STRIP)
        buf = np.frombuffer(fbo.read(components=3), np.uint8).reshape(r.h, r.w, 3)[::-1]
        ff.stdin.write(buf.tobytes())
        if f % 60 == 0:
            print(f"  frame {f}/{frames}", flush=True)
    ff.stdin.close()
    print("encode rc", ff.wait(), "->", args.out)


if __name__ == "__main__":
    main()
