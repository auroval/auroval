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
