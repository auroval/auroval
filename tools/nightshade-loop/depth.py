"""Estimate a monocular depth map for a scene master.

Depth-Anything-V2-**Small** is the only variant that can be used here: it is
Apache-2.0, while the Base and Large checkpoints are CC-BY-NC-4.0, which would
make anything derived from them unshippable. The quality difference does not
matter for what this depth is used for, which is a few tens of pixels of
parallax rather than a reconstruction.

    python3 tools/nightshade-loop/depth.py MASTER.png OUT.npy
"""

import argparse

import cv2
import numpy as np
from PIL import Image

MODEL = "depth-anything/Depth-Anything-V2-Small-hf"


def estimate(master: np.ndarray) -> np.ndarray:
    """Return a 0..1 depth map the same size as `master` (RGB, any bit depth)."""
    import torch
    from transformers import pipeline

    eight = master if master.dtype == np.uint8 else (master / 257).astype(np.uint8)
    pipe = pipeline("depth-estimation", model=MODEL, device=0 if torch.cuda.is_available() else -1)
    d = np.asarray(pipe(Image.fromarray(eight))["depth"], np.float32)
    d = cv2.resize(d, (master.shape[1], master.shape[0]), interpolation=cv2.INTER_CUBIC)
    return (d - d.min()) / max(float(d.max() - d.min()), 1e-6)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("master", help="scene master, 8- or 16-bit PNG")
    ap.add_argument("out", help="where to write the .npy depth map")
    ap.add_argument("--preview", help="optional 8-bit PNG to write alongside it")
    args = ap.parse_args()

    bgr = cv2.imread(args.master, cv2.IMREAD_UNCHANGED)
    if bgr is None:
        raise SystemExit(f"cannot read {args.master}")
    depth = estimate(cv2.cvtColor(bgr, cv2.COLOR_BGR2RGB))
    np.save(args.out, depth)
    if args.preview:
        cv2.imwrite(args.preview, (depth * 255).astype(np.uint8))
    print(f"depth {depth.shape[1]}x{depth.shape[0]} -> {args.out}")


if __name__ == "__main__":
    main()
