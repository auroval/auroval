-- A theme that gives R9's fallback chain explicitly: the video first, a
-- still behind it, and the solid colour behind that by running out.
--
-- The single-string form in good.lua is the common case and still works;
-- this is the one that could not be written before.
return {
  background = {
    "/usr/share/auroval/nightshade/loop.mp4",
    "/usr/share/auroval/nightshade/still.png",
  },
}
