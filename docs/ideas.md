# Parked ideas

Things worth keeping but not committing to. Nothing here is a requirement, and
nothing here belongs in an early phase.

### Themes as WebAssembly

> **Status after Q4, 2026-11-07: still parked, and deliberately so.** Q4 chose
> Lua in a restricted interpreter, which spends part of the argument below but
> not the strongest part. The *capability* this idea was reaching for — letting
> a theme's behaviour run at all — now exists, so the question is no longer
> "how could a theme run?" but "is a Lua sandbox a good enough boundary?".
> Today the answer is yes for one user running themes he wrote himself. It
> would stop being yes the moment themes are shared between strangers, which
> is the condition to watch for. See
> [the Q4 decision](decisions/q4-configuration-format.md).

The maintainer's own framing: not an obligation, and certainly not phase one —
worth doing only if it unlocks something that gives a genuine wow effect in one
move.

What it would and would not buy, from a look at the state of it:

- **Not a configuration language.** WebAssembly is a compilation target, not
  something you write. Expressing colours and sizes in it would mean a Rust
  toolchain to change a shade of blue, and no precedent for it was found.
  Declarative look belongs in a text format with line numbers and good error
  messages.
- **It is a way to let a theme *run*.** The thesis already says a theme
  determines *"hoe die switchen"* — how backgrounds switch. That is behaviour,
  not data, and no config format expresses it. This is the real argument.
- **Sandboxing is the strongest part**, and it is specific to this project — and
  it is the part Q4 did *not* spend. A restricted Lua interpreter denies
  capabilities by removing them from the environment; a WebAssembly instance
  cannot reach them because the host never handed them over. Those are different
  kinds of guarantee, and only the second survives a mistake in the setup code. A
  theme obtained from the internet would be running inside a greeter, a
  pre-authentication process on a locked screen. WebAssembly grants no
  filesystem or network access unless the host hands it over; an embedded
  scripting language hands over the whole process. It is the difference between
  a theme that can be shared safely and one that cannot.
- **The cost is measurable and modest where it matters.** Instantiation is
  microseconds once a module is compiled ahead of time; compiling at load is
  what costs seconds, so any design must precompile. A published cold-start
  measurement including compilation sits around 17 ms. The size the runtime adds
  to the binary is **unverified** — no source found.

The honest shape, if it ever happens: declarative look in a text format,
executable theme behaviour in WebAssembly, and only once there is something for
that behaviour to do. A procedural or generative background that computes itself
rather than playing a file is the candidate most likely to justify it.

Prior art for the pattern, not the use: Zellij's plugins are WebAssembly/WASI,
and `zjstatus` — a status bar — is written as one. Zellij's own documentation
calls its plugin system early and its API a work in progress.

### Live motion: compositing depth layers in the renderer

Parked on 2026-11-07, when the maintainer decided **bake** — see
[the §7 note](themes/nightshade-assets.md) and issue #18.

The idea: a theme ships separated depth layers plus motion parameters, and the
renderer composites each frame with sine-driven parallax instead of decoding a
video. No decoder on any surface, no loop seam even in principle, assets a
fraction of the size, and the desktop's subtle drift costing almost nothing.

**Why it is parked rather than rejected.** Tenet 3 is a real argument for it:
*"het moet tijdloos zijn en niet gaan irriteren"*, and a 10–30 second loop will
eventually be recognised as a loop. Recognising the loop point is exactly the
kind of thing that grates by the second week. Sine-driven parallax has no loop
point to recognise, so this is not a performance optimisation dressed up — it
addresses the tenet the project is least able to test in advance.

**Why it is not the architecture.** Tenet 6 settles that: *"mensen moeten zelf
ook eigen video's in kunnen gebruiken"*. An arbitrary MP4 has no depth layers in
it, so live compositing cannot serve the media the project has committed to
supporting. The decoder is therefore mandatory, and live motion would add a
second motion path alongside it rather than removing anything. Every constraint
it appeared to retire — C6's memory-type portability, Q5's renderer difficulty,
R8, C15's registry cost — stays required for the user-video path regardless.

**The trigger to revisit, stated so it is falsifiable.** If living with a baked
loop shows loop recognisability to be the irritation tenet 3 warns about — which
gate 3 of the asset brief is two weeks of exactly this test — then this becomes
worth building as an *additional* path for shipped themes, with user video still
decoded. Nothing short of that observation should reopen it, because the
alternative is building a second engine to fix a problem nobody has had yet.

**If it is ever built: motion parameters, not a motion function.** A theme
supplying amplitude, frequency, phase and parallax depth per layer is data, the
maths is Rust, and R22 stays a load-time concern with §3.3's fallbacks. A theme
supplying a motion *function* runs it sixty times a second in a
pre-authentication process under a restricted interpreter, which turns every one
of R22's failure modes into a per-frame concern. The first is buildable; the
second is the attractive-sounding one.
