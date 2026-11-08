-- R22, mode one: a theme that raises.
--
-- The realistic shape of this is not a deliberate `error()` call but a typo
-- that indexes a nil, so that is what this does first. Either way the
-- greeter's job is to catch it rather than die with it.
local palette = nil
return {
  primary = palette.primary,
}
