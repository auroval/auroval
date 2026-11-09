# Misbehaving themes, kept on purpose

These are R22's fit criterion, committed. Each file misbehaves in exactly one
of the four ways the requirement names, and each has a test in
`src/config.rs` asserting that it still yields a usable `Config`.

They are in the repository rather than written inline in the test because a
requirement whose test cannot be re-run is a requirement that quietly rots:
anybody can point the greeter at one of these by hand and watch it shrug.

| file | misbehaves by | stopped by |
|---|---|---|
| `raises.lua` | calling `error()` | the error being caught rather than propagated |
| `loops-forever.lua` | `while true do end` | the instruction ceiling |
| `allocates-forever.lua` | growing a table without bound | the memory ceiling |
| `reaches-out.lua` | `io.open`, `os.execute`, `require` | those names never being bound |
| `broken-syntax.lua` | not being valid Lua | the parser, which names the line |
| `good.lua` | nothing — the control | n/a |
