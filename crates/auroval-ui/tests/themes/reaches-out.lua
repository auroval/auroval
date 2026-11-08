-- R22, mode four: a theme that reaches for the filesystem and a process.
--
-- R22's wording is exact: this must fail *at the call* rather than succeeding.
-- So the assertion that matters is not merely that the config falls back to
-- defaults -- a theme could fail for any number of boring reasons -- but that
-- the shadow file was never opened and `id` was never run.
local f = io.open("/etc/shadow", "r")
local out = os.execute("id")
local m = require("socket")
return { background = tostring(f) .. tostring(out) .. tostring(m) }
