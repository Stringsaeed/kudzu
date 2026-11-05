//! A small ICU / `DateFormatter` pattern formatter (no ICU dependency).
//!
//! Supports the common fields (era, years, months, days, weekdays, quarters, ISO weeks,
//! hours, minutes, seconds, fractions, AM/PM and numeric/GMT zones). Names are localized for
//! en, fr, de, es, it, pt, nl and ar; other locales fall back to English. Digits are always
//! Latin and zone names are GMT offsets, since only a fixed UTC offset is known.

use chrono::{DateTime, Datelike, FixedOffset, Timelike};

/// Formats `date` with ICU `pattern` in `locale`.
pub(crate) fn format(date: &DateTime<FixedOffset>, pattern: &str, locale: &str) -> String {
    let locale = Locale::parse(locale);
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::with_capacity(pattern.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            if chars.get(i + 1) == Some(&'\'') {
                out.push('\'');
                i += 2;
                continue;
            }
            i += 1;
            while i < chars.len() {
                if chars[i] == '\'' {
                    if chars.get(i + 1) == Some(&'\'') {
                        out.push('\'');
                        i += 2;
                        continue;
                    }
                    i += 1;
                    break;
                }
                out.push(chars[i]);
                i += 1;
            }
            continue;
        }
        if c.is_ascii_alphabetic() {
            let mut count = 1;
            while chars.get(i + count) == Some(&c) {
                count += 1;
            }
            field(&mut out, date, c, count, &locale);
            i += count;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn field(
    out: &mut String,
    date: &DateTime<FixedOffset>,
    letter: char,
    count: usize,
    locale: &Locale,
) {
    let names = locale.names;
    let hour = date.hour();
    match letter {
        'G' => {
            let index = usize::from(date.year() > 0);
            match count {
                1..=3 => out.push_str(names.eras[index]),
                4 => out.push_str(names.eras_full[index]),
                _ => out.push_str(&first_char_upper(names.eras[index])),
            }
        }
        'y' => {
            let year = era_year(date.year());
            if count == 2 {
                pad(out, i64::from(year % 100), 2);
            } else {
                pad(out, i64::from(year), count);
            }
        }
        'Y' => {
            let year = date.iso_week().year();
            if count == 2 {
                pad(out, i64::from(year.rem_euclid(100)), 2);
            } else {
                pad(out, i64::from(year), count);
            }
        }
        'u' => pad(out, i64::from(date.year()), count),
        'M' | 'L' => {
            let month = date.month0() as usize;
            match count {
                1 | 2 => pad(out, month as i64 + 1, count),
                3 => out.push_str(names.months_abbr[month]),
                5 => out.push_str(&first_char_upper(names.months[month])),
                _ => out.push_str(names.months[month]),
            }
        }
        'd' => pad(out, i64::from(date.day()), count),
        'D' => pad(out, i64::from(date.ordinal()), count),
        'F' => pad(out, i64::from((date.day() - 1) / 7 + 1), count),
        'E' => weekday_name(out, date, count, names),
        'e' | 'c' => {
            if count <= 2 {
                let weekday = date.weekday();
                let local = if locale.sunday_first {
                    weekday.num_days_from_sunday()
                } else {
                    weekday.num_days_from_monday()
                };
                pad(
                    out,
                    i64::from(local + 1),
                    if letter == 'e' { count } else { 1 },
                );
            } else {
                weekday_name(out, date, count, names);
            }
        }
        'a' | 'b' | 'B' => out.push_str(if hour < 12 { names.am } else { names.pm }),
        'h' => pad(
            out,
            i64::from(if hour.is_multiple_of(12) {
                12
            } else {
                hour % 12
            }),
            count,
        ),
        'H' => pad(out, i64::from(hour), count),
        'k' => pad(out, i64::from(if hour == 0 { 24 } else { hour }), count),
        'K' => pad(out, i64::from(hour % 12), count),
        'm' => pad(out, i64::from(date.minute()), count),
        's' => pad(out, i64::from(date.second()), count),
        'S' => {
            let digits = format!("{:09}", date.nanosecond() % 1_000_000_000);
            if count <= 9 {
                out.push_str(&digits[..count]);
            } else {
                out.push_str(&digits);
                out.extend(std::iter::repeat_n('0', count - 9));
            }
        }
        'A' => {
            let millis = i64::from(date.num_seconds_from_midnight()) * 1000
                + i64::from(date.nanosecond() / 1_000_000);
            pad(out, millis, count);
        }
        'Q' | 'q' => {
            let quarter = date.month0() / 3 + 1;
            match count {
                1 | 5 => pad(out, i64::from(quarter), 1),
                2 => pad(out, i64::from(quarter), 2),
                3 => {
                    out.push('Q');
                    pad(out, i64::from(quarter), 1);
                }
                _ => out.push_str(
                    ["1st quarter", "2nd quarter", "3rd quarter", "4th quarter"]
                        [quarter as usize - 1],
                ),
            }
        }
        'w' => pad(out, i64::from(date.iso_week().week()), count),
        'W' => {
            let first = date
                .with_day(1)
                .map_or(0, |d| d.weekday().num_days_from_monday());
            pad(out, i64::from((date.day() - 1 + first) / 7 + 1), count);
        }
        'Z' => {
            let seconds = date.offset().local_minus_utc();
            match count {
                1..=3 => offset(out, seconds, false, false),
                4 => gmt(out, seconds, true),
                _ => {
                    if seconds == 0 {
                        out.push('Z');
                    } else {
                        offset(out, seconds, true, false);
                    }
                }
            }
        }
        'O' | 'z' | 'v' | 'V' => {
            let seconds = date.offset().local_minus_utc();
            gmt(out, seconds, count >= 4 || (letter == 'V' && count >= 2));
        }
        'x' | 'X' => {
            let seconds = date.offset().local_minus_utc();
            if letter == 'X' && seconds == 0 {
                out.push('Z');
            } else {
                match count {
                    1 => offset(out, seconds, false, true),
                    2 | 4 => offset(out, seconds, false, false),
                    _ => offset(out, seconds, true, false),
                }
            }
        }
        _ => out.extend(std::iter::repeat_n(letter, count)),
    }
}

fn weekday_name(out: &mut String, date: &DateTime<FixedOffset>, count: usize, names: &Names) {
    let weekday = date.weekday().num_days_from_sunday() as usize;
    match count {
        1..=3 => out.push_str(names.weekdays_abbr[weekday]),
        4 => out.push_str(names.weekdays[weekday]),
        5 => out.push_str(&first_char_upper(names.weekdays[weekday])),
        _ => out.extend(
            names.weekdays_abbr[weekday]
                .chars()
                .filter(|c| *c != '.')
                .take(2),
        ),
    }
}

/// ICU `y` is the year of era: 1 BC is year 1.
fn era_year(year: i32) -> i32 {
    if year <= 0 {
        1 - year
    } else {
        year
    }
}

fn pad(out: &mut String, value: i64, width: usize) {
    if value < 0 {
        out.push('-');
    }
    let digits = value.unsigned_abs().to_string();
    out.extend(std::iter::repeat_n('0', width.saturating_sub(digits.len())));
    out.push_str(&digits);
}

fn first_char_upper(s: &str) -> String {
    s.chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default()
}

/// `+HHmm` / `+HH:mm`; `short` drops zero minutes (`+05`).
fn offset(out: &mut String, seconds: i32, colon: bool, short: bool) {
    let minutes_total = seconds.abs() / 60;
    let (h, m) = (minutes_total / 60, minutes_total % 60);
    out.push(if seconds < 0 { '-' } else { '+' });
    out.push_str(&format!("{h:02}"));
    if short && m == 0 {
        return;
    }
    if colon {
        out.push(':');
    }
    out.push_str(&format!("{m:02}"));
}

/// Localized GMT format: `GMT`, `GMT+5`, `GMT+5:30` (short) or `GMT+05:00` (long).
fn gmt(out: &mut String, seconds: i32, long: bool) {
    out.push_str("GMT");
    if seconds == 0 {
        return;
    }
    if long {
        offset(out, seconds, true, false);
        return;
    }
    let minutes_total = seconds.abs() / 60;
    let (h, m) = (minutes_total / 60, minutes_total % 60);
    out.push(if seconds < 0 { '-' } else { '+' });
    out.push_str(&h.to_string());
    if m != 0 {
        out.push_str(&format!(":{m:02}"));
    }
}

struct Locale {
    names: &'static Names,
    sunday_first: bool,
}

impl Locale {
    fn parse(identifier: &str) -> Locale {
        let mut parts = identifier.split(['_', '-', '@', '.']);
        let language = parts.next().unwrap_or("").to_ascii_lowercase();
        let region = parts
            .find(|p| p.len() == 2 || p.len() == 3)
            .map(str::to_ascii_uppercase);
        let names = match language.as_str() {
            "fr" => &FR,
            "de" => &DE,
            "es" => &ES,
            "it" => &IT,
            "pt" => &PT,
            "nl" => &NL,
            "ar" => &AR,
            _ => &EN,
        };
        let sunday_first = match region.as_deref() {
            Some(r) => [
                "US", "CA", "JP", "BR", "MX", "IL", "KR", "TW", "PH", "IN", "ZA",
            ]
            .contains(&r),
            None => language == "en" || language.is_empty(),
        };
        Locale {
            names,
            sunday_first,
        }
    }
}

struct Names {
    months: [&'static str; 12],
    months_abbr: [&'static str; 12],
    /// Sunday first.
    weekdays: [&'static str; 7],
    weekdays_abbr: [&'static str; 7],
    am: &'static str,
    pm: &'static str,
    /// BC, AD.
    eras: [&'static str; 2],
    eras_full: [&'static str; 2],
}

static EN: Names = Names {
    months: [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ],
    months_abbr: [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ],
    weekdays: [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ],
    weekdays_abbr: ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"],
    am: "AM",
    pm: "PM",
    eras: ["BC", "AD"],
    eras_full: ["Before Christ", "Anno Domini"],
};

static FR: Names = Names {
    months: [
        "janvier",
        "février",
        "mars",
        "avril",
        "mai",
        "juin",
        "juillet",
        "août",
        "septembre",
        "octobre",
        "novembre",
        "décembre",
    ],
    months_abbr: [
        "janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.",
        "déc.",
    ],
    weekdays: [
        "dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi",
    ],
    weekdays_abbr: ["dim.", "lun.", "mar.", "mer.", "jeu.", "ven.", "sam."],
    am: "AM",
    pm: "PM",
    eras: ["av. J.-C.", "ap. J.-C."],
    eras_full: ["avant Jésus-Christ", "après Jésus-Christ"],
};

static DE: Names = Names {
    months: [
        "Januar",
        "Februar",
        "März",
        "April",
        "Mai",
        "Juni",
        "Juli",
        "August",
        "September",
        "Oktober",
        "November",
        "Dezember",
    ],
    months_abbr: [
        "Jan.", "Feb.", "März", "Apr.", "Mai", "Juni", "Juli", "Aug.", "Sept.", "Okt.", "Nov.",
        "Dez.",
    ],
    weekdays: [
        "Sonntag",
        "Montag",
        "Dienstag",
        "Mittwoch",
        "Donnerstag",
        "Freitag",
        "Samstag",
    ],
    weekdays_abbr: ["So.", "Mo.", "Di.", "Mi.", "Do.", "Fr.", "Sa."],
    am: "AM",
    pm: "PM",
    eras: ["v. Chr.", "n. Chr."],
    eras_full: ["v. Chr.", "n. Chr."],
};

static ES: Names = Names {
    months: [
        "enero",
        "febrero",
        "marzo",
        "abril",
        "mayo",
        "junio",
        "julio",
        "agosto",
        "septiembre",
        "octubre",
        "noviembre",
        "diciembre",
    ],
    months_abbr: [
        "ene", "feb", "mar", "abr", "may", "jun", "jul", "ago", "sept", "oct", "nov", "dic",
    ],
    weekdays: [
        "domingo",
        "lunes",
        "martes",
        "miércoles",
        "jueves",
        "viernes",
        "sábado",
    ],
    weekdays_abbr: ["dom", "lun", "mar", "mié", "jue", "vie", "sáb"],
    am: "a. m.",
    pm: "p. m.",
    eras: ["a. C.", "d. C."],
    eras_full: ["antes de Cristo", "después de Cristo"],
};

static IT: Names = Names {
    months: [
        "gennaio",
        "febbraio",
        "marzo",
        "aprile",
        "maggio",
        "giugno",
        "luglio",
        "agosto",
        "settembre",
        "ottobre",
        "novembre",
        "dicembre",
    ],
    months_abbr: [
        "gen", "feb", "mar", "apr", "mag", "giu", "lug", "ago", "set", "ott", "nov", "dic",
    ],
    weekdays: [
        "domenica",
        "lunedì",
        "martedì",
        "mercoledì",
        "giovedì",
        "venerdì",
        "sabato",
    ],
    weekdays_abbr: ["dom", "lun", "mar", "mer", "gio", "ven", "sab"],
    am: "AM",
    pm: "PM",
    eras: ["a.C.", "d.C."],
    eras_full: ["avanti Cristo", "dopo Cristo"],
};

static PT: Names = Names {
    months: [
        "janeiro",
        "fevereiro",
        "março",
        "abril",
        "maio",
        "junho",
        "julho",
        "agosto",
        "setembro",
        "outubro",
        "novembro",
        "dezembro",
    ],
    months_abbr: [
        "jan.", "fev.", "mar.", "abr.", "mai.", "jun.", "jul.", "ago.", "set.", "out.", "nov.",
        "dez.",
    ],
    weekdays: [
        "domingo",
        "segunda-feira",
        "terça-feira",
        "quarta-feira",
        "quinta-feira",
        "sexta-feira",
        "sábado",
    ],
    weekdays_abbr: ["dom.", "seg.", "ter.", "qua.", "qui.", "sex.", "sáb."],
    am: "AM",
    pm: "PM",
    eras: ["a.C.", "d.C."],
    eras_full: ["antes de Cristo", "depois de Cristo"],
};

static NL: Names = Names {
    months: [
        "januari",
        "februari",
        "maart",
        "april",
        "mei",
        "juni",
        "juli",
        "augustus",
        "september",
        "oktober",
        "november",
        "december",
    ],
    months_abbr: [
        "jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec",
    ],
    weekdays: [
        "zondag",
        "maandag",
        "dinsdag",
        "woensdag",
        "donderdag",
        "vrijdag",
        "zaterdag",
    ],
    weekdays_abbr: ["zo", "ma", "di", "wo", "do", "vr", "za"],
    am: "a.m.",
    pm: "p.m.",
    eras: ["v.Chr.", "n.Chr."],
    eras_full: ["voor Christus", "na Christus"],
};

static AR: Names = Names {
    months: [
        "يناير",
        "فبراير",
        "مارس",
        "أبريل",
        "مايو",
        "يونيو",
        "يوليو",
        "أغسطس",
        "سبتمبر",
        "أكتوبر",
        "نوفمبر",
        "ديسمبر",
    ],
    months_abbr: [
        "يناير",
        "فبراير",
        "مارس",
        "أبريل",
        "مايو",
        "يونيو",
        "يوليو",
        "أغسطس",
        "سبتمبر",
        "أكتوبر",
        "نوفمبر",
        "ديسمبر",
    ],
    weekdays: [
        "الأحد",
        "الاثنين",
        "الثلاثاء",
        "الأربعاء",
        "الخميس",
        "الجمعة",
        "السبت",
    ],
    weekdays_abbr: [
        "الأحد",
        "الاثنين",
        "الثلاثاء",
        "الأربعاء",
        "الخميس",
        "الجمعة",
        "السبت",
    ],
    am: "ص",
    pm: "م",
    eras: ["ق.م", "م"],
    eras_full: ["قبل الميلاد", "ميلادي"],
};
