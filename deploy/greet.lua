-- auroval's theme on this machine: nightshade, one scene, with the palette
-- that scene derives to.
--
-- Installed by hand to /etc/auroval/greet.lua -- see TRIAL.md §1. `install.nu`
-- does not write it, for the same reason it does not write the PAM lines:
-- which scenes a machine shows is a per-machine choice, and §2's rollback
-- should not leave this project's file behind on a machine back on SDDM.
--
-- The OUTER list is a pool of candidates, which are equals; `rotation` picks
-- one per launch. Each INNER list is R9's fallback chain for that candidate:
-- the loop first, its still behind it, the solid colour behind that by
-- running out. The two orders mean different things -- §10.10 of
-- docs/themes/nightshade-assets.md -- which is why they are nested.
--
-- WHY THE POOL IS ONE DEEP. scene-05 is installed and deliberately left out,
-- because `palette` is one table for the whole theme while `background` is a
-- pool, and these two scenes do not share a hue:
--
--     scene-01   hue 213.5 deg   primary 00bcd0   C* 38.4   (cyan)
--     scene-05   hue  38.7 deg   primary ff3522   C* 93.5   (red-orange)
--
-- Measured 2026-11-22 with `tools/scene-check/scene_check.py` against the
-- installed stills. With both in the pool, half of all boots would show the
-- lantern under a cyan accent -- which is the disagreement nightshade.md §4.4
-- derives the palette at packaging time to prevent, arriving through the
-- other door. Q11's sub-question is which shape fixes that; until it is
-- answered, one scene is the honest pool. Uncomment the second candidate to
-- see the fault.
return {
  background = {
    {
      "/usr/local/share/auroval/nightshade-01.mp4",
      "/usr/local/share/auroval/nightshade-01.png",
    },
    -- {
    --   "/usr/local/share/auroval/nightshade-05.mp4",
    --   "/usr/local/share/auroval/nightshade-05.png",
    -- },
  },

  -- scene-01's palette, derived once from its canonical frame per §4.4, not
  -- re-derived at login. Bare six-digit hex with no leading `#`, so callers
  -- append their own alpha byte. `tertiary` is what colours the caps-lock
  -- check; `surface` supplies a hue and not the input field's translucency.
  -- `error` is fixed by the theme across every scene -- a failure message
  -- must not change colour with the wallpaper (§4.3).
  palette = {
    primary    = "00bcd0",
    on_surface = "d6ecf0",
    outline    = "7a8f93",
    surface    = "142224",
    tertiary   = "8eb8f8",
    error      = "ffb4ab",
  },
}
