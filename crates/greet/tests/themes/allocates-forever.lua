-- R22, mode three: a theme that allocates without bound.
--
-- Strings rather than table entries, because concatenation grows the heap
-- faster than it burns instructions and so this races the memory ceiling to
-- the finish rather than the instruction ceiling. The point is to prove the
-- memory ceiling fires, which means it has to be the one that fires.
local t = {}
local chunk = string.rep("a", 8192)
local i = 1
while true do
  t[i] = chunk .. tostring(i)
  i = i + 1
end
