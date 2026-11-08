//! The clock and date line, in the machine's own language.
//!
//! hyprlock gets this for free: its date label is `cmd[] date +"%A, %-d %B"`,
//! and `date` honours `LC_TIME` like any other POSIX program. This greeter
//! formats with chrono, whose default is English, so on a machine with
//! `LC_TIME=nl_NL.UTF-8` the lock screen said *zondag, 8 november* and the
//! login screen said *Sunday, 8 November*. Nobody would notice that on one
//! screen; it is obvious the moment the two are side by side, which is how it
//! was found on 2026-11-08.
//!
//! That is exactly the seam the vision is about — one consistent picture —
//! so the greeter resolves the locale the way the C library does, from
//! `LC_ALL`, then `LC_TIME`, then `LANG`.
//!
//! **The greeter's environment is not your shell's.** It runs as `greeter`
//! under greetd, where those variables are whatever systemd exported from
//! `/etc/locale.conf` and nothing else. When none of them is set, or names a
//! locale chrono does not carry, the fallback is POSIX, which is English —
//! the same thing `date` would print in the same situation.

use chrono::{DateTime, Local, Locale};

/// hyprlock's own two formats, from the config this greeter matches.
const CLOCK: &str = "%H:%M";
const DATE: &str = "%A, %-d %B";

/// Resolve the date locale from the environment, as the C library does.
///
/// `LC_ALL` wins over `LC_TIME`, which wins over `LANG`. An empty value is
/// not a setting — `LC_ALL=` is how a caller says "ignore this one" — so it
/// falls through rather than resolving to the empty locale.
pub fn locale_from_env() -> Locale {
    for key in ["LC_ALL", "LC_TIME", "LANG"] {
        if let Some(v) = std::env::var_os(key) {
            let v = v.to_string_lossy().to_string();
            if !v.is_empty() {
                return parse_locale(&v);
            }
        }
    }
    Locale::POSIX
}

/// `nl_NL.UTF-8` -> `nl_NL`. Pure, so the mapping is testable without
/// setting process-wide environment variables from a test thread.
///
/// A locale name is `language[_TERRITORY][.codeset][@modifier]`, and chrono
/// knows only the first two parts, so the codeset and modifier are cut. `C`
/// and `POSIX` are the C library's names for "no localisation" and both land
/// on English, which is what they mean.
fn parse_locale(raw: &str) -> Locale {
    let base = raw.split(['.', '@']).next().unwrap_or(raw).trim();
    match base {
        "" | "C" | "POSIX" => Locale::POSIX,
        name => Locale::try_from(name).unwrap_or(Locale::POSIX),
    }
}

/// The two strings the view draws, in one call so they cannot drift apart.
pub fn lines(now: DateTime<Local>, locale: Locale) -> (String, String) {
    (
        now.format_localized(CLOCK, locale).to_string(),
        now.format_localized(DATE, locale).to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn a_sunday() -> DateTime<Local> {
        // 2026-11-08 was a Sunday, which is the day the mismatch was found.
        Local.with_ymd_and_hms(2026, 11, 8, 8, 43, 0).unwrap()
    }

    #[test]
    fn the_date_is_written_in_the_locales_language() {
        let (clock, date) = lines(a_sunday(), Locale::nl_NL);
        assert_eq!(clock, "08:43");
        assert_eq!(date, "zondag, 8 november");
    }

    #[test]
    fn posix_is_english_because_that_is_what_posix_means() {
        let (_, date) = lines(a_sunday(), Locale::POSIX);
        assert_eq!(date, "Sunday, 8 November");
    }

    #[test]
    fn the_codeset_and_modifier_are_not_part_of_the_name() {
        assert_eq!(parse_locale("nl_NL.UTF-8"), Locale::nl_NL);
        assert_eq!(parse_locale("de_DE.UTF-8@euro"), Locale::de_DE);
        assert_eq!(parse_locale("nl_NL"), Locale::nl_NL);
    }

    #[test]
    fn c_and_posix_and_nonsense_all_fall_back_to_english() {
        // The fallback matters more than the lookup: a greeter that refuses
        // to draw a date because the locale is unknown is worse than one
        // that draws it in English.
        for raw in ["C", "POSIX", "", "not_a_locale", "zz_ZZ.UTF-8"] {
            assert_eq!(parse_locale(raw), Locale::POSIX, "{raw}");
        }
    }
}
