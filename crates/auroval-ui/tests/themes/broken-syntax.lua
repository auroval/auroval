-- R13: a config that is not valid Lua at all.
--
-- `palette` is given no value, so Lua gives up at the closing brace on the
-- last line. The test derives that line number from the file rather than
-- hardcoding it, so this comment can be reworded without silently turning the
-- assertion into a check on the wrong line.
return {
  background = "/usr/share/auroval/still.png",
  palette =
}
