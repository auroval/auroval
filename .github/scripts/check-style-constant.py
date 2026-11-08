#!/usr/bin/env python3
"""The nightshade rendering style is written down three times. Check they agree.

nightshade-assets.md §4.1.1 says a theme has exactly one rendering style and
that it "is not negotiable per scene", because "two scenes drawn in different
styles do not read as one theme no matter how well they match on palette,
lighting and composition".

That string therefore has to be identical everywhere it appears, and it
appears in three places for three different reasons:

  docs/themes/nightshade-assets.md  §4.1.1, the authority, with the date it
                                    was chosen and the six candidates it beat
  tools/nightshade-gen/generate.py  STYLE, what is actually sent to the model
  each scene's prompt.json          the record beside the image

The first two are in this repository and are checked here. The third lives
with the assets, which are not committed, so it cannot be.

Why this is a CI step and not a comment: generate.py's constants are
transcribed from the document, generate.py's own comment admits that
"transcription can drift from its source", and a drift here is close to
invisible. It would not fail anything. It would produce a scene whose style
clause is subtly not the theme's, and nobody would notice until two images
were put side by side and did not look related -- which is weeks later, at
gate 3, with nothing pointing at the cause.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DOC = ROOT / "docs/themes/nightshade-assets.md"
GEN = ROOT / "tools/nightshade-gen/generate.py"


def from_doc() -> str:
    """§4.1.1's operative string: consecutive backticked blockquote lines.

    The document gives it as two lines so it fits the page, and says plainly
    why the string is recorded at all rather than the art-historical label:
    "it is what actually reproduces the look and an art-historical label is
    not".
    """
    lines = re.findall(r"^> `(.+)`$", DOC.read_text(), re.MULTILINE)
    if not lines:
        sys.exit(f"{DOC}: found no backticked blockquote lines for the style")
    return " ".join(line.strip() for line in lines)


def from_generator() -> str:
    # Imported rather than parsed, so that what is checked is the value the
    # generator would actually use, including its string concatenation.
    sys.path.insert(0, str(GEN.parent))
    from generate import STYLE

    return STYLE


def main() -> int:
    doc, gen = from_doc(), from_generator()
    if doc == gen:
        print(f"style constant agrees in both places ({len(doc)} chars)")
        return 0
    print("the nightshade rendering style has drifted:", file=sys.stderr)
    print(f"  {DOC.relative_to(ROOT)}\n    {doc!r}", file=sys.stderr)
    print(f"  {GEN.relative_to(ROOT)}\n    {gen!r}", file=sys.stderr)
    print(
        "\nThe document is the authority (nightshade-assets.md §4.1.1).\n"
        "Change the generator to match it, not the other way round -- and if\n"
        "the style is genuinely being changed, that is a new theme, which is\n"
        "what §4.1.1's closing paragraph is about.",
        file=sys.stderr,
    )
    return 1


if __name__ == "__main__":
    sys.exit(main())
