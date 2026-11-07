-- R22, mode two: a theme that never returns.
--
-- The instruction ceiling stops this. The test asserts not only that it is
-- stopped but that it is stopped *quickly*, because "terminated" and "does not
-- delay the first prompt" are two separate claims and only the second one is
-- what a person at the login screen actually experiences.
local n = 0
while true do
  n = n + 1
end
return { background = "never reached" }
