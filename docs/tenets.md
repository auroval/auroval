# Tenets

Tie-breakers, not values. Each one names a choice between two genuine goods, so
that an argument six months from now is already settled. Every tenet states its
cost, because a tenet without a cost is a slogan.

Drawn from an interview with the maintainer; his wording is quoted where it is
the content rather than a summary of it.

---

### 1. A whole theme over an adjustable one

**When customisation conflicts with coherence, coherence wins.**

Changing one element is not supported. Wanting something different means
authoring a theme, and a theme determines the backgrounds, how they switch, and
what goes in the bar.

**Because** *"het is een beetje alles of niets als het om customization gaat"* —
a thousand independent settings is how a desktop ends up looking like four
programs that have never met.

**Cost** Someone who wants one different colour is told to write a theme. That
is a steep answer to a small request, it will be asked repeatedly, and it will
cost casual users who would otherwise have stayed.

---

### 2. Motion you can see over motion you infer

**When visible movement conflicts with cheap movement, visible wins.**

A background that moves while you look at it comes before a background that
tracks the time of day and the seasons. Both are wanted; this is the order.

**Because** *"dan is het toch de beweging die wauw geeft."*

**Cost** The premise rested on an assumption nobody had verified: that hardware
video decode works inside the greeter's context, where there is no user session
and no session runtime directory. Time-of-day switching would have been nearly
free and carried no such risk. This tenet knowingly took the expensive road.

**Expires if** the P0 spike shows decode cannot work there. Then motion is a
lock-screen feature only and this tenet inverts.

**Status: survives.** The P0 spike ran on 2026-11-07 and the condition did not
trigger. As greetd's own user, with no session, no runtime directory and no
environment at all, a 1440p60 loop decodes in hardware at 59.5 fps for 2.5% of
one core, with 9.2× headroom. See PRD C6.

The expensive road turned out to be cheap, which is worth saying plainly rather
than quietly banking: this tenet was right for a reason it did not know at the
time. What the spike found instead was a different and smaller cost — the
greeter has nowhere to cache, so a login pays about 0.9 s of plugin rescanning
unless it is given a writable directory (PRD C15). That is a deployment detail,
not a reason to invert anything.

One narrower risk remains, and it is not this machine's: the path that works is
NVDEC, and a machine where VA-API is the only option is untested. If this ever
runs on AMD or Intel hardware, this tenet is unverified there.

---

### 3. The fiftieth look over the first

**When first-impression impact conflicts with living with it, living with it
wins.**

**Because** *"het moet tijdloos zijn en niet gaan irriteren."*

**Cost** This rules out most of what demos well. Effects that make a good
screenshot are usually the ones that grate by the second week, so the project
will look less impressive in exactly the places where people decide whether to
try it.

---

### 4. The maintainer's daily use over a newcomer's first hour

**When the maintainer's own experience conflicts with being easy for strangers,
the maintainer wins.**

**Because** *"als niemand anders het dan ik het installeer, dan is het niet
dood... Ik heb het druk zat."* An absent audience is information, not failure.

**Cost** Documentation stays thin. The configuration format carries no
stability promise. Issues are answered when convenient. A contributor who
arrives expecting a maintained project may well leave again, and that is
accepted rather than mitigated.

**Expires if** a contributor turns up who would carry the project. Then this
inverts, and it inverts loudly, because the cost above is only acceptable while
there is one user.

---

### 5. The empty surface over the crowded one

**When effort could go to the bar or to the login screen, the login screen
wins.**

**Because** eight projects already build a Hyprland bar and several do it well,
while none of them is a greeter. The login screen is also where the proof lives:
*"dat ik echt het gevoel heb dat ik gewoon in één consistent logisch plaatje
zit."*

**Cost** The suite stays incomplete for longer, and anyone evaluating it early
finds a login screen and no desktop — which is the least persuasive possible
order to build this in.

---

### 6. Your own media over curated defaults

**When supporting arbitrary user media conflicts with guaranteeing it always
looks right, the user's own file wins.**

**Because** *"mensen moeten zelf ook eigen video's in kunnen gebruiken. Dus niet
per se gebonden zijn aan één of twee video's die meegeleverd worden."*

**Cost** Arbitrary files mean arbitrary formats, resolutions, aspect ratios,
colour spaces and broken files, all of which must degrade gracefully rather than
fail. Shipping two videos that always work would have been far less effort and
would have looked better in every screenshot.
