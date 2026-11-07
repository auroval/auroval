# Vision

An opinionated look for Hyprland: the login screen, the lock screen, the bar and
the window overview as one set of parts that agree with each other.

This document was written from an interview with the maintainer. Where his own
words say it better than a summary would, they are quoted in Dutch with a
rendering alongside. Lines not his are marked.

## Where it started

Sitting at the machine, and seeing it:

> "dat heel blauwe scherm en daar zag ik ook een leeg profile picture en een
> echt veel te klein, super lelijk inputbox die hoekig was en compleet iets
> anders deed dan de rest van mijn omgeving"

*That very blue screen, and an empty profile picture, and a really far too
small, super ugly input box with square corners that did something completely
different from the rest of my environment.*

Three of those four complaints are details. The fourth is the system, and it is
the one that matters: the login screen did something completely different from
everything else on the machine.

What was wanted instead:

> "iets wat gewoon meteen een gevoel geeft van wauw, wat super cool is dit"

*something that immediately gives you a feeling of: wow, this is seriously
cool.*

## The thesis

**A theme is a whole, not a pile of settings.**

> "ik wil voornamelijk dat mensen niet ad hoc kleine elementen kunnen
> veranderen, maar dat alles echt een consistent thema moet zijn. Dus als iemand
> iets wil customizen, dan moet hij een eigen thema hebben. En dat bepaalt dan
> ook meteen de set van achtergronden, hoe die switchen, wat er in de bar staat,
> gewoon het hele pakket. Dus het is een beetje alles of niets als het om
> customization gaat."

*I mainly want people not to be able to change small elements ad hoc; everything
really has to be one consistent theme. So if someone wants to customise, they
need their own theme. And that then also determines the set of backgrounds, how
they switch, what goes in the bar — just the whole package. So it is a bit of
all or nothing when it comes to customisation.*

Most projects in this space believe the opposite: that more settings is more
freedom. This one holds that a thousand independent settings is how a desktop
ends up looking like four programs that have never met — which is exactly the
state the login screen was found in.

The inspiration is Apple's lock and login screen, explicitly for its behaviour
and not its taste: *"dat is consistent en het ziet er gewoon super mooi uit met
die parallax achtergrond, met die beweging. Zoiets dergelijks zou ik ook willen,
maar dan qua stijl heel anders."*

## The scene

The background moves while you look at it. Asked to choose between movement you
can see in the moment and a screen that quietly tracks the time of day and the
seasons, the answer was immediate:

> "dan is het toch de beweging die wauw geeft"

*then it is the movement that gives the wow.*

Time of day remains wanted — *"meebeweegt met de tijd van de dag op zijn minst,
maar idealiter zelfs ook de seizoenen"* — but second. The moving background is
the thing; the clock and the calendar are how it stays interesting.

And it has to survive being looked at every day:

> "het moet tijdloos zijn en niet gaan irriteren"

*it has to be timeless and not become irritating.*

Backgrounds are not curated. People bring their own: *"mensen moeten zelf ook
eigen video's in kunnen gebruiken. Dus niet per se gebonden zijn aan één of twee
video's die meegeleverd worden."*

## What it is not

- **Not a settings panel.** No changing one colour, one font, one corner radius.
  Customisation means authoring a theme, and a theme covers everything.
- **Not a project that needs an audience.** If nobody else installs it, that is
  information, not failure:

  > "als niemand anders het dan ik het installeer, dan is het niet dood. Dan ben
  > ik er gewoon achter dat er geen interesse is in wat ik bedacht heb. En daar
  > kan ik prima mee leven. Ik heb het druk zat."

  *If nobody but me installs it, it is not dead. Then I have simply found out
  there is no interest in what I came up with. And I can live with that fine. I
  am busy enough.*

- **Not another Hyprland bar first.** Wayle, Noctalia, Way-Shell, Ax-Shell,
  Caelestia, DankMaterialShell, Modus and OkPanel all build that, several of
  them well. None of them is a greeter.
- **Not Apple's taste.** Its consistency and its motion, deliberately not its
  styling.

## The proof

Not a number. A moment:

> "dat ik zie dat de greeter en het lockscreen heel consistent zijn met de
> desktop experience. Dat ik echt het gevoel heb dat ik gewoon in één consistent
> logisch plaatje zit."

*that I see the greeter and the lock screen are really consistent with the
desktop experience. That I genuinely feel I am inside one consistent, logical
picture.*

> [inferred] Which puts the first component where the proof is: the login and
> lock screens are not merely the first thing built, they are the test of the
> whole idea. A bar that matches a desktop is unremarkable; a login screen that
> does is the point.
