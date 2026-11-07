# P0 spike results — hardware decode in greetd's context

Verbatim output of `spike/p0/greeter-decode-test.nu`, kept as the receipt
behind PRD constraints C6 and C15. Phase 2 (video on screen under `cage`,
with a real greetd login) is reported at the bottom.

---

## Phase 1 — hardware decode as greetd's own user

Run 2026-11-07 11:28:09 +0100 on games-werkkamer.

Method: the spike binary is run via `runuser -u greeter` with `env -i`,
so it has uid 950, no session, no XDG_RUNTIME_DIR, no D-Bus and no
driver hints. Media and binary are staged world-readable under
`/var/lib/auroval-spike`, outside any home directory.

## Versions

```
Linux 7.3.1-1-cachyos
greetd 0.10.3-2.1
cage 0.3.1-1.1
gstreamer 1.28.8-1
gst-plugins-bad 1.28.8-1
gst-plugin-va 1.28.8-1
libva-nvidia-driver 0.0.18-1.1
nvidia-utils 615.78.08-2
rustc 1.94.1 (e408947bf 2026-03-25)
```

## Context and GStreamer registry

```
## Process context

- uid/gid: 950/950
- user: greeter
- supplementary groups: 950 983

## Session environment

- `XDG_RUNTIME_DIR` = *(unset)*
- `XDG_SESSION_TYPE` = *(unset)*
- `XDG_SESSION_ID` = *(unset)*
- `WAYLAND_DISPLAY` = *(unset)*
- `DISPLAY` = *(unset)*
- `GREETD_SOCK` = *(unset)*
- `DBUS_SESSION_BUS_ADDRESS` = *(unset)*
- `LIBVA_DRIVER_NAME` = *(unset)*
- `GST_PLUGIN_PATH` = *(unset)*
- `GST_VAAPI_ALL_DRIVERS` = *(unset)*
- `__GLX_VENDOR_LIBRARY_NAME` = *(unset)*

## Device access (as this process, right now)

- `/dev/dri/renderD128`: **rw**
- `/dev/dri/card1`: **rw**
- `/dev/nvidia0`: **rw**
- `/dev/nvidiactl`: **rw**
- `/dev/nvidia-modeset`: **rw**
- `/dev/nvidia-uvm`: **rw**

## GStreamer

- version: GStreamer 1.28.8
- `nvh264dec`: present, rank 257, instantiates: yes
- `nvh265dec`: present, rank 257, instantiates: yes
- `vah264dec`: **absent**
- `vah265dec`: **absent**
- `avdec_h264`: present, rank 256, instantiates: yes
- `avdec_h265`: present, rank 256, instantiates: yes
```

## loop1440p60.hevc.mp4, path=auto, realtime

```
--- result ---
path          auto
mode          realtime (sync=true)
file          /var/lib/auroval-spike/media/loop1440p60.hevc.mp4
frames        1194
wall          20.00s
fps           59.70
cpu           0.59s (2.9% of one core)
loops         1
late buffers  0
elements      appsink -> decodebin3 -> parsebin -> filesrc -> qtdemux -> h265parse -> capsfilter -> parsebin -> typefind -> h265parse -> capsfilter -> nvh265dec
hw decoder    nvh265dec
gpu memory    yes
caps          video/x-raw(memory:CUDAMemory), format=(string)NV12, width=(int)2560, height=(int)1440, interlace-mode=(string)progressive, multiview-mode=(string)mono, multiview-flags=(GstVideoMultiviewFlagsSet)0:ffffffff:/right-view-first/left-flipped/left-flopped/right-flipped/right-flopped/half-aspect/mixed-mono, pixel-aspect-ratio=(fraction)1/1, framerate=(fraction)60/1
error         none
```

## loop1440p60.hevc.mp4, path=sw, realtime

```
--- result ---
path          software
mode          realtime (sync=true)
file          /var/lib/auroval-spike/media/loop1440p60.hevc.mp4
frames        1197
wall          20.00s
fps           59.85
cpu           10.80s (54.0% of one core)
loops         1
late buffers  0
elements      appsink -> avdec_h265 -> h265parse -> qtdemux -> filesrc
hw decoder    NONE (software)
gpu memory    no (frames copied to system memory)
caps          video/x-raw, format=(string)I420, width=(int)2560, height=(int)1440, interlace-mode=(string)progressive, pixel-aspect-ratio=(fraction)1/1, chroma-site=(string)mpeg2, colorimetry=(string)2:0:0:0, framerate=(fraction)60/1
error         none
```

## loop1440p60.h264.mp4, path=auto, realtime

```
--- result ---
path          auto
mode          realtime (sync=true)
file          /var/lib/auroval-spike/media/loop1440p60.h264.mp4
frames        1190
wall          20.00s
fps           59.50
cpu           0.51s (2.5% of one core)
loops         1
late buffers  0
elements      appsink -> decodebin3 -> parsebin -> filesrc -> qtdemux -> h264parse -> capsfilter -> parsebin -> typefind -> h264parse -> capsfilter -> nvh264dec
hw decoder    nvh264dec
gpu memory    yes
caps          video/x-raw(memory:CUDAMemory), format=(string)NV12, width=(int)2560, height=(int)1440, interlace-mode=(string)progressive, multiview-mode=(string)mono, multiview-flags=(GstVideoMultiviewFlagsSet)0:ffffffff:/right-view-first/left-flipped/left-flopped/right-flipped/right-flopped/half-aspect/mixed-mono, pixel-aspect-ratio=(fraction)1/1, framerate=(fraction)60/1
error         none
```

## loop1440p60.h264.mp4, path=sw, realtime

```
--- result ---
path          software
mode          realtime (sync=true)
file          /var/lib/auroval-spike/media/loop1440p60.h264.mp4
frames        1198
wall          20.00s
fps           59.90
cpu           7.28s (36.4% of one core)
loops         1
late buffers  0
elements      appsink -> avdec_h264 -> h264parse -> qtdemux -> filesrc
hw decoder    NONE (software)
gpu memory    no (frames copied to system memory)
caps          video/x-raw, format=(string)I420, width=(int)2560, height=(int)1440, interlace-mode=(string)progressive, pixel-aspect-ratio=(fraction)1/1, chroma-site=(string)mpeg2, framerate=(fraction)60/1
error         none
```

## Headroom (sync=false, decode ceiling)

```
--- result ---
path          auto
mode          headroom (sync=false)
file          /var/lib/auroval-spike/media/loop1440p60.h264.mp4
frames        10993
wall          20.00s
fps           549.65
cpu           1.13s (5.6% of one core)
loops         15
late buffers  0
elements      appsink -> decodebin3 -> parsebin -> filesrc -> qtdemux -> h264parse -> capsfilter -> parsebin -> typefind -> h264parse -> capsfilter -> nvh264dec
hw decoder    nvh264dec
gpu memory    yes
caps          video/x-raw(memory:CUDAMemory), format=(string)NV12, width=(int)2560, height=(int)1440, interlace-mode=(string)progressive, multiview-mode=(string)mono, multiview-flags=(GstVideoMultiviewFlagsSet)0:ffffffff:/right-view-first/left-flipped/left-flopped/right-flipped/right-flopped/half-aspect/mixed-mono, pixel-aspect-ratio=(fraction)1/1, framerate=(fraction)60/1
error         none
```

## Startup: GStreamer registry cost (R16)

Process start to GStreamer ready, measured three ways. `cold` has a
writable cache but no registry yet; `warm` reuses it; the third has
nowhere to write, which is what greetd's greeter does by default
because its home is `/`.

```
cold (registry absent, must rescan): 1sec 11ms 61µs 252ns
warm (registry cached)             : 92ms 947µs 349ns
no writable cache at all           : 1sec 18ms 188µs 812ns
```

---

## Phase 2 — video on screen under `cage`

`spike-p0 greet` run under `cage` nested in Hyprland, as uid 1000, with a warm
GStreamer registry. The surface size is the nested window's, not the monitor's.

```
surface configured    : 1255x674, 71ms after process start
first decoded frame   : 232ms after start
first presented frame : 243ms after start
frames decoded        : 1490 over 25.1s  (59.4 fps)
frames presented      : 1488
font                  : /usr/share/fonts/TTF/MesloLGSNerdFontMono-Regular.ttf
pipeline              : filesrc ! qtdemux ! h264parse ! nvh264dec
                        ! cudaconvertscale ! cudadownload
                        ! video/x-raw,format=BGRx,width=1255,height=674
                        ! appsink max-buffers=1 drop=true sync=true
```

Two caveats on this figure, so it is not read as more than it is. Presentation
goes through `wl_shm`, which means a GPU-to-CPU readback and one copy per frame
— deliberately not what R7 ultimately wants, because choosing a renderer is Q5
and the spike must not prejudge it. And the greetd login was exercised
separately: `GREETD_SOCK` is unset under a nested `cage`, so this run proves the
decode-and-present path, not the auth path.

---

## Portability — VA-API measured against NVDEC

GStreamer's `va` plugin registers **zero features** on this machine despite
`libva` working and `gst-plugin-va` being installed. The cause is explicit, not
a misconfiguration:

```
vadisplay gstvadisplay_drm.c:157: DRM render node with kernel driver nvidia-drm
vadisplay gstvadisplay.c:176:     VA-API driver vendor: VA-API NVDEC driver [direct backend]
vadisplay gstvadisplay.c:186: WARN Unsupported driver: VA-API NVDEC driver [direct backend]
```

`_gst_va_display_filter_driver` matches the driver's vendor string against an
allowlist and rejects NVIDIA's VA-API shim by name. `GST_VA_ALL_DRIVERS=1`
bypasses the filter, after which `vah264dec`, `vah265dec`, `vaav1dec`,
`vavp9dec` and others appear — which is how the comparison below was obtained.

Same clip, 1440p60 H.264 High, 20 s, realtime:

| | NVDEC `nvh264dec` | VA-API `vah264dec` (forced) |
|---|---|---|
| rate | 59.50 fps | 59.90 fps |
| late buffers | 0 | 0 |
| CPU | 2.5% of one core | 3.8% |
| frame memory | `memory:CUDAMemory` | plain `video/x-raw` (system memory) |

So VA-API works and holds rate, but copies frames down — failing R7's
no-readback budget. The filter is a sound judgement rather than an oversight.

With the filter bypassed, `decodebin3` *still* selects `nvh264dec`, because
`nvcodec` ranks 257 against `va`'s lower rank. Rank selection therefore gets
this right unaided, which is the whole portability argument: name no decoder.

Reproduce:

```nu
GST_VA_ALL_DRIVERS=1 target/release/spike-p0 decode <file.mp4> --path vaapi --seconds 20
```

This also makes the AMD/Intel code path exercisable on this machine, which is
useful for P4 — with the caveat that it exercises NVIDIA's shim, not Mesa or
`iHD`, so DMABuf export behaviour is still unmeasured.
