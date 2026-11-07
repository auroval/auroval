-- The control: a theme that behaves, so that the tests which prove the
-- misbehaving ones degrade are not quietly proving that nothing works at all.
return {
  background = "/usr/share/auroval/nightshade/still.png",
  state_path = "/var/lib/auroval/greet-test",
  palette = {
    primary = "00bdce",
    on_surface = "d6ecf0",
    tertiary = "cba6f7",
  },
  geometry = {
    clock_px = 96,
    field_w = 420,
  },
}
