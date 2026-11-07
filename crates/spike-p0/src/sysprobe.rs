//! Facts about the process's own context — the things C6's unverified half is
//! actually about.

use std::fmt::Write as _;

/// CPU time this process has burned, user + system, in seconds.
///
/// Read from `/proc/self/stat` rather than `getrusage` so that the figure is
/// the same one `top` would show, and so the reader can check it by hand.
pub fn cpu_seconds() -> f64 {
    let Ok(stat) = std::fs::read_to_string("/proc/self/stat") else {
        return f64::NAN;
    };
    // The comm field may contain spaces and parentheses, so fields are counted
    // from the last ')' rather than from the start.
    let Some(after_comm) = stat.rsplit_once(')').map(|(_, rest)| rest) else {
        return f64::NAN;
    };
    let f: Vec<&str> = after_comm.split_whitespace().collect();
    // After ')' the first field is `state` (field 3), so utime (14) and stime
    // (15) are at indices 11 and 12.
    let ticks: f64 = match (f.get(11), f.get(12)) {
        (Some(u), Some(s)) => u.parse::<f64>().unwrap_or(0.0) + s.parse::<f64>().unwrap_or(0.0),
        _ => return f64::NAN,
    };
    let hz = 100.0; // _SC_CLK_TCK is 100 on every Linux target in practice.
    ticks / hz
}

fn can_open(path: &str) -> &'static str {
    use std::fs::OpenOptions;
    match OpenOptions::new().read(true).write(true).open(path) {
        Ok(_) => "rw",
        Err(_) => match OpenOptions::new().read(true).open(path) {
            Ok(_) => "ro",
            Err(_) => "DENIED",
        },
    }
}

/// Everything about this process's context that could plausibly decide whether
/// hardware decode works. Printed verbatim into the spike's report so the
/// verdict is reproducible rather than asserted.
pub fn report() -> String {
    let mut s = String::new();
    let _ = writeln!(s, "## Process context");
    let _ = writeln!(s);
    let _ = writeln!(s, "- uid/gid: {}/{}", unsafe { libc_getuid() }, unsafe {
        libc_getgid()
    });
    let _ = writeln!(
        s,
        "- user: {}",
        std::env::var("USER").unwrap_or_else(|_| "<unset>".into())
    );
    let _ = writeln!(s, "- supplementary groups: {}", groups_str());
    let _ = writeln!(s);
    let _ = writeln!(s, "## Session environment");
    let _ = writeln!(s);
    for k in [
        "XDG_RUNTIME_DIR",
        "XDG_SESSION_TYPE",
        "XDG_SESSION_ID",
        "WAYLAND_DISPLAY",
        "DISPLAY",
        "GREETD_SOCK",
        "DBUS_SESSION_BUS_ADDRESS",
        "LIBVA_DRIVER_NAME",
        "GST_PLUGIN_PATH",
        "GST_VAAPI_ALL_DRIVERS",
        "__GLX_VENDOR_LIBRARY_NAME",
    ] {
        let _ = writeln!(
            s,
            "- `{k}` = {}",
            match std::env::var(k) {
                Ok(v) => format!("`{v}`"),
                Err(_) => "*(unset)*".into(),
            }
        );
    }
    let _ = writeln!(s);
    let _ = writeln!(s, "## Device access (as this process, right now)");
    let _ = writeln!(s);
    for dev in [
        "/dev/dri/renderD128",
        "/dev/dri/card1",
        "/dev/nvidia0",
        "/dev/nvidiactl",
        "/dev/nvidia-modeset",
        "/dev/nvidia-uvm",
    ] {
        if std::path::Path::new(dev).exists() {
            let _ = writeln!(s, "- `{dev}`: **{}**", can_open(dev));
        } else {
            let _ = writeln!(s, "- `{dev}`: *(absent)*");
        }
    }
    s
}

fn groups_str() -> String {
    // Read from /proc rather than getgroups() to avoid a libc dependency for
    // the sake of one line.
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|st| {
            st.lines()
                .find(|l| l.starts_with("Groups:"))
                .map(|l| l.trim_start_matches("Groups:").trim().to_string())
        })
        .unwrap_or_else(|| "<unknown>".into())
}

// Avoiding a `libc` dependency for two integers.
extern "C" {
    #[link_name = "getuid"]
    fn libc_getuid() -> u32;
    #[link_name = "getgid"]
    fn libc_getgid() -> u32;
}
