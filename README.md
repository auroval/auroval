# auroval

An opinionated look for Hyprland, as one set of parts that agree with each
other: the login screen, the lock screen, the bar, the window overview, and the
wallpaper-derived palette that ties them together.

**Status: design phase, with the P0 spike done.** The one assumption the whole
project rested on — that hardware video decode works inside a greeter, where
there is no user session and the process is not the logging-in user — has been
measured and holds: 1440p60 in hardware for 2.5% of one core, with 9.2× to
spare. See [PRD C6](docs/prd/greet.md) and the receipts in
[`spike/p0/RESULTS.md`](spike/p0/RESULTS.md). The code so far is that spike plus
the greetd protocol implementation it needed; there is no `greet` yet.

## Why a suite, and why start at the login screen

Hyprland has no shortage of desktop shells — Wayle, Noctalia, Way-Shell,
Ax-Shell, Caelestia, DankMaterialShell, Modus, OkPanel and others all build a
bar with notifications and a launcher, several of them well.

None of them is a greeter. The login screen is where a themed desktop gives
itself away: it is the one surface that belongs to no session, so it falls back
to whatever the display manager ships. auroval starts there and works outward,
which is the opposite order from every project listed above and the reason this
one exists.

The components share one renderer, one configuration language (hyprlang, which
Hyprland and hyprlock already use) and one palette. That sharing is the whole
argument for a mono-repo: a greeter and a bar that merely *look* alike will
drift apart, while two front-ends over one renderer cannot.

## Components

| | |
|---|---|
| `greet` | Login screen (greetd) and lock screen (`ext-session-lock-v1`), one binary, animated background on both. [PRD](docs/prd/greet.md) |
| `bar` | Status bar. Not yet specified. |
| `expose` | Window overview. Not yet specified. |
| `theme` | Wallpaper-derived palette, generated with matugen, consumed by every component. |

## Read this first

[`docs/vision.md`](docs/vision.md) and [`docs/tenets.md`](docs/tenets.md) come
first and win over anything below them. The tenets are tie-breakers with their
costs stated — read those if you want to know what this project refuses to do
and why. [`docs/ideas.md`](docs/ideas.md) holds what is deliberately parked.

[`docs/prd/greet.md`](docs/prd/greet.md) is the requirements document for the
first component. It states the goals and, more usefully, the non-goals, the
constraints with their sources, and the assumptions that are still
**unverified** — one of which decides whether the component is viable at all.

If you are evaluating whether this project should exist, read its "Ruled out"
section. It includes a working alternative that needs no code.

## The website

[`site/`](site/) is auroval.org, built with Zola. It is one page today — what
the suite is, the tenets with their costs, and the palette rule with its
measurements — and the documentation under `docs/` is the obvious thing it
grows into, which is why it is a generator and not a hand-written page.

Two of its files are **generated and must not be edited**: `site/sass/_tokens.scss`
and `site/static/gamut.json` both come out of
[`tools/scene-check/emit_web_tokens.py`](tools/scene-check/emit_web_tokens.py),
which derives them with the same `build_palette()` the theme uses. CI fails if
either is stale. Regenerate with:

```
python3 tools/scene-check/emit_web_tokens.py --write
```

Zola itself is **not installed on the host**. It is pinned and checksummed in
[`.devcontainer/zola-version.txt`](.devcontainer/zola-version.txt) and installed
into the devcontainer image and onto CI runners by the same script, so the two
cannot drift. Inside the container:

```
zola --root site serve
```

The brand's one fixed hue, and what choosing it cost, is
[`docs/decisions/b1-brand-hue.md`](docs/decisions/b1-brand-hue.md).

### Hosting

**GitHub Pages**, published by
[`.github/workflows/pages.yml`](.github/workflows/pages.yml) on every push to
`main` that touches the site or the generator. `ci.yml` builds the site too, but
only as a pull-request gate — the two are separate so that a workflow a fork can
trigger never holds the permissions a deploy needs.

The custom domain is **not** recorded in a `CNAME` file. GitHub reads one only
when Pages publishes from a branch; with a custom workflow it is ignored, and a
`CNAME` in the repository neither adds nor removes a domain. The setting lives in
**Settings → Pages → Custom domain**, set to `auroval.org`, with *Enforce HTTPS*
on once the certificate has been issued.

The DNS at the registrar, for the apex:

| Type | Name | Value |
|------|------|-------|
| A | `@` | `185.199.108.153` |
| A | `@` | `185.199.109.153` |
| A | `@` | `185.199.110.153` |
| A | `@` | `185.199.111.153` |
| AAAA | `@` | `2606:50c0:8000::153` |
| AAAA | `@` | `2606:50c0:8001::153` |
| AAAA | `@` | `2606:50c0:8002::153` |
| AAAA | `@` | `2606:50c0:8003::153` |
| CNAME | `www` | `auroval.github.io.` |
| MX | `@` | `0 .` |
| TXT | `@` | `v=spf1 -all` |
| TXT | `_dmarc` | `v=DMARC1; p=reject; sp=reject; adkim=s; aspf=s` |

Those are GitHub's published apex addresses, confirmed against
`pages.github.com` when this was written. They change rarely but they do change:
check
[GitHub's own documentation](https://docs.github.com/en/pages/configuring-a-custom-domain-for-your-github-pages-site/managing-a-custom-domain-for-your-github-pages-site)
before trusting this table rather than the other way round. All four records of
each family are wanted — they are not alternatives.

**The last three records say that this domain has nothing to do with mail**, and
they are there on purpose rather than by omission. The registrar's default zone
had an `MX` pointing at the apex, which is fine while the apex is the
registrar's own hosting and silently wrong the moment it is GitHub's: GitHub
runs no SMTP, so mail would be sent to a machine that cannot refuse it
properly.

Deleting the `MX` is not enough either. With no `MX` at all, a sending server
falls back to the `A` record — straight back at GitHub — and the mail sits in a
retry queue for days before bouncing. `MX 0 .` is a null MX
([RFC 7505](https://www.rfc-editor.org/rfc/rfc7505)): it says *this domain
accepts no mail*, and a sender rejects immediately instead of trying. `v=spf1
-all` and `p=reject` say the matching thing in the other direction — nobody is
authorised to send as `auroval.org` — which is worth having for a name that
appears on a project page, because a domain that visibly sends no mail is a
poor one to forge. The registrar's default `v=spf1 ~all` said *"probably not"*,
which is not a useful thing to say.

If a mailbox is ever wanted on this domain, all three come out first, before
anything is pointed anywhere.

`.app`, `.nl` and `.online` are redirects and need no records here; they point at
`.org` at the registrar.

## Licence

**MIT OR Apache-2.0**, at your option. See [`LICENSE-MIT`](LICENSE-MIT) and
[`LICENSE-APACHE`](LICENSE-APACHE).

This is the Rust ecosystem's usual dual licence, and nothing in the project
forced it either way. Open question Q2 had raised the possibility that the
`greetd_ipc` crate — GPL-3.0-only, declared in its own manifest though not
shown by docs.rs — would pull the authentication path into GPL. It would have,
so the protocol is implemented in-house in `crates/greetd-proto` instead. An
audit of the remaining 113 crates in the dependency graph found **no copyleft
at all**: everything is MIT, Apache-2.0, Zlib or Unlicense, and the C libraries
underneath (GStreamer and its plugins) are LGPL-2.1-or-later, which constrains
nothing that links them dynamically.

So the choice was free, and permissive is the one that matches
[tenet 4](docs/tenets.md): copyleft's value is enforcement, and enforcement is
work this project has already said it will not do. Apache-2.0 is offered
alongside MIT for its express patent grant, which is worth having in a project
built on hardware video decode.

**A caveat that the licence does not cover.** Apache-2.0's patent grant
concerns contributions to *this* software. It says nothing about H.264 or HEVC
codec patents, which are a separate matter entirely. auroval ships no decoder —
GStreamer and the GPU driver do the decoding — so this is somebody else's
licensing problem, but it is not solved by anything in this file.
