# A1 — which image generator?

Status: **open, for the maintainer.** Recommendation below; the decision is his.
`nightshade-assets.md` §9, item A1: *"Which generator, and is its licence
compatible with shipping the output? Not settled, and it gates everything."*

The maintainer has said a cloud generator is acceptable and that quality
matters more than speed — *"it would be cool if we can reach high quality with
it even if it has to run a night."* That reframes the question usefully, because
it turns out neither the quality nor the runtime is what decides this.

## What the brief actually requires

Two constraints, both from the project's own documents rather than from
preference.

**A1 asks about the licence of the output, not of the tool.** The repository is
public and `MIT OR Apache-2.0`. Scene assets ship in it. So the generator's
terms have to permit redistributing its output under those terms — which is a
narrower question than "may I use this commercially".

**`source.md` has a stated purpose that is not provenance.** §3: *"`source.md`
is not optional. Without it the set cannot be extended in the same voice a year
later."* It records prompt, model, seed and settings so that **scene four, made
a year after scene one, still looks like the same theme.** That is a
reproducibility requirement with a one-year horizon, and it is the thing that
decides A1.

## The case against the cloud, on this project's terms

Not a general objection. Three specific ones.

**1. Hosted models are retired on a schedule, and the schedule is short.**
Imagen 1 and 2 were removed in September 2025. Imagen 4 preview models in
November 2025. Imagen 4 GA was deprecated with migration recommended before
30 June 2026, and trackers list an August 2026 shutdown, replaced by
`gemini-3.1-flash-image`. Current Gemini image models carry retirement dates in
2027.

So on past cadence, the model that produces scene one is likely to be
unavailable before scene four. `source.md` would faithfully record a model name
that no longer resolves — provenance preserved, **reproducibility gone**, which
is the half the document says it exists for. A theme whose later scenes cannot
match its earlier ones fails tenet 1 at the level of the artwork rather than the
config.

**2. Redistribution under an open-source licence is unanswered.** Google's
public position on Imagen output is an assurance that customers own the IP from
their models — spoken in a 2023 launch article, not contract language — and
nothing located states whether that output may be redistributed under a
permissive licence by a third party. For a public repository, "probably fine"
is not an answer to A1, and the authoritative text is the Vertex AI service
terms rather than anything summarised here.

**3. The watermark lands in the one place this pipeline is known to be
fragile.** Vertex adds a digital watermark to generated images and it cannot be
disabled from the console. §5.4 of the assets brief records a *measured* finding
that matugen's hue extraction swings by up to **118°** on the same pixels at
different bit depths. An invisible, uncontrolled signal injected into images
whose palette derivation we have already proven to be bit-depth sensitive is a
variable in exactly the wrong place. It may well be harmless; nothing in the
project can currently show that it is.

## Why "it can run a night" argues *for* local

This is the part that inverts. Overnight compute is the cheapest thing a local
GPU offers and the most expensive thing a hosted API sells, and gates 1 and 2
in §6 are **rejection** gates — their job is to throw candidates away. A
pipeline whose value comes from generating many candidates and discarding most
of them wants the marginal image to cost nothing.

Overnight, locally, is also where the quality gap closes: high step counts,
multiple seeds per prompt, and a refinement or upscale pass on the survivors.
A 4B model given a night and three passes is a different proposition from a 4B
model given twenty seconds.

## Local option, verified

**FLUX.2 [klein] 4B**, Black Forest Labs.

- **Apache-2.0 on the weights**, per BFL's own model card, with outputs usable
  commercially. That answers A1's licence half cleanly and compatibly with this
  repository's own licence.
- **Fits.** BFL's table puts it near 8.4 GB; the model card says around 13 GB.
  `fp8` and `nvfp4` builds and GGUF quantisations exist, so a 4070 Ti with
  12 GB has room — at `fp8` with margin for a refinement pass.
- **Reproducible from a pinned revision hash**, because the weights are held
  locally. This is the only property on this page that a hosted model cannot
  offer at any price, and it is the one `source.md` requires.

**A trap worth naming: the 9B klein is the better model and is
non-commercially licensed.** Only the 4B variants are Apache-2.0. Reaching for
9B because it looks better would make the shipped assets unredistributable,
which is precisely the failure A1 is asking about.

## Recommendation

**Generate what ships locally, with FLUX.2 [klein] 4B at `fp8`, pinned by
revision hash.** Explore in the cloud as much as is useful — it costs the
project nothing and hosted models are genuinely better at the top end.

One honest caveat on that split: a look found with a frontier hosted model may
not be reachable with a local 4B one, so cloud exploration should be treated as
a **mood board and not a target**. Choosing the shipped look in the cloud and
then trying to reproduce it locally is the one way this recommendation goes
wrong.

## What this also decides

**A5 becomes cheaper.** A5 asks whether layer separation for §7 comes from the
generator or is cut by hand per master, and §7's offline pipeline still needs
those layers after the bake decision. Locally, depth estimation and inpainting
run in the same pipeline on the same image, so separation is a step rather than
a project. A hosted generator returns a flat image and leaves all of A5 to be
solved afterwards.

## Sources

- [FLUX.2 [klein] 4B model card](https://huggingface.co/black-forest-labs/FLUX.2-klein-4B)
- [FLUX.2 [klein] 4B fp8](https://huggingface.co/black-forest-labs/FLUX.2-klein-4b-fp8)
- [BFL's FLUX.2 [klein] page](https://bfl.ai/models/flux-2-klein)
- [Vertex AI generative AI release notes](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/release-notes)
- [Vertex AI model versions and lifecycle](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/learn/model-versions)
- [Imagen on Vertex AI, image generation](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/image/generate-images)
