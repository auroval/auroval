-- A nested Hyprland, so `auroval-lock rehearse` locks something that is not
-- the session you are sitting in.
--
--   Hyprland -c spike/p3/nested.lua
--
-- THIS FILE IS LUA, AND THE REASON IS WORTH A PARAGRAPH. It was
-- `nested.conf`, in hyprlang, which Hyprland has deprecated since 0.55 --
-- the maintainer's own config was migrated to Lua long before this file was.
-- Hyprland 0.57.1 reads a `-c` file as Lua whatever its extension, so every
-- `#` comment in the old file was a syntax error on line 2, the config was
-- abandoned whole, and the compositor came up in emergency mode. Nothing
-- below was in effect. That was invisible for two reasons: the error appears
-- as an overlay on the nested window and not in the log, and a compositor
-- running on its defaults still locks and still renders, so the rehearsals
-- looked fine. What gave it away was two runs reporting different surface
-- sizes -- 630x339 at scale 2 and 1920x1080 at scale 1 -- because the
-- monitor line below was never applied. Caught 2026-11-09, by the operator
-- reading his own screen.
--
-- The settings matter more than they did when the file was written. L9 took
-- away the escape key and the timer's unlock, so a rehearsal now ends with
-- the compositor still locked and YOU CLOSE THIS WINDOW. There is no key.
-- On the session you are sitting in there would be no way back but the right
-- password, or killing the compositor from another VT and losing everything
-- unsaved.

-- One output, with the scale pinned to 1 so that the size this reports is
-- the size it draws. It does NOT pin the pixel count: a nested Wayland
-- output follows the window the host gives it, so resizing that window
-- changes the figure, and 1920x1080 and 1259x678 have both been seen. Only
-- the scale is settled here. Absolute numbers across runs are still not
-- comparable -- which is what lock-measure.nu means by THE PAIRING IS THE
-- POINT, and why it runs both front ends against one compositor.
hl.monitor({
    output   = "WAYLAND-1",
    mode     = "preferred",
    position = "0x0",
    scale    = 1,
})

-- Everything that moves is off. The measurement is of this project's present
-- path, and a compositor animating its own windows underneath would be in
-- the number.
hl.config({
    misc = {
        disable_hyprland_logo    = true,
        disable_splash_rendering = true,
        force_default_wallpaper  = 0,
    },
    animations = { enabled = false },
    decoration = { blur = { enabled = false } },
})

-- A valid config registers no binds unless it says so, and a nested
-- compositor with no binds has no keyboard way out at all -- the emergency
-- binds that covered this while the config was broken are gone the moment it
-- parses. Closing the window is still the documented way out, and it is the
-- one that works while the screen is locked; this is here for a run that
-- ended with the compositor unlocked and focus inside it.
hl.bind("SUPER + M", hl.dsp.exit())
