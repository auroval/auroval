-- A theme that offers two candidates to rotate between, each with its own
-- R9 chain behind it. The outer list is a pool of equals; the inner ones are
-- ordered by degradation. chain.lua is the same file with one candidate.
--
-- The second candidate is deliberately a different length, because a pool of
-- equal-length chains cannot catch code that confuses the two dimensions.
return {
  background = {
    { "/var/lib/auroval/nightshade-01.mp4", "/var/lib/auroval/nightshade-01.png" },
    { "/var/lib/auroval/nightshade-05.mp4" },
  },
}
