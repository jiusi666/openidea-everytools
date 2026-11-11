//! 日期算法：civil_from_days / days_from_civil，不依赖 chrono。
//!
//! 时间戳工具与 Cron 共用。所有「现在」由面板传入（毫秒 UTC），
//! WASM 侧不依赖 WASI 时钟，宿主没实现 wall-clock 也不会崩。

pub const DAY_MS: i64 = 86_400_000;
pub const WEEK_CN: [&str; 7] = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"];

/// 1970-01-01 起的天数 → (年, 月, 日)。
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

/// 0 = 周日 … 6 = 周六。
pub fn weekday_from_days(z: i64) -> u32 {
    (z + 4).rem_euclid(7) as u32
}

pub fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

pub fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 30,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parts {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub weekday: u32,
    pub day_index: i64,
}

pub fn local_parts(ms: i64, tz_min: i64) -> Parts {
    let shifted = ms + tz_min * 60_000;
    let day_index = shifted.div_euclid(DAY_MS);
    let rem = shifted.rem_euclid(DAY_MS);
    let (year, month, day) = civil_from_days(day_index);
    Parts {
        year,
        month,
        day,
        hour: (rem / 3_600_000) as u32,
        minute: ((rem / 60_000) % 60) as u32,
        second: ((rem / 1000) % 60) as u32,
        weekday: weekday_from_days(day_index),
        day_index,
    }
}

pub fn ms_from_local(
    year: i64,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    tz_min: i64,
) -> i64 {
    days_from_civil(year, month, day) * DAY_MS
        + (hour as i64 * 3600 + minute as i64 * 60 + second as i64) * 1000
        - tz_min * 60_000
}

pub fn format_parts(p: &Parts) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02} {}",
        p.year, p.month, p.day, p.hour, p.minute, p.second, WEEK_CN[p.weekday as usize]
    )
}

pub fn format_ms(ms: i64, tz_min: i64) -> String {
    format_parts(&local_parts(ms, tz_min))
}

pub fn tz_label(tz_min: i64) -> String {
    let sign = if tz_min < 0 { '-' } else { '+' };
    let abs = tz_min.abs();
    format!("UTC{}{:02}:{:02}", sign, abs / 60, abs % 60)
}

pub fn relative(ms: i64, now: i64) -> String {
    let diff = now - ms;
    let abs = diff.abs();
    let (n, unit) = if abs < 60_000 {
        (abs / 1000, "秒")
    } else if abs < 3_600_000 {
        (abs / 60_000, "分钟")
    } else if abs < DAY_MS {
        (abs / 3_600_000, "小时")
    } else if abs < 30 * DAY_MS {
        (abs / DAY_MS, "天")
    } else if abs < 365 * DAY_MS {
        (abs / (30 * DAY_MS), "个月")
    } else {
        (abs / (365 * DAY_MS), "年")
    };
    if n == 0 {
        "刚刚".to_string()
    } else if diff > 0 {
        format!("{n} {unit}前")
    } else {
        format!("{n} {unit}后")
    }
}

/// 解析日期字符串（不支持时返回 None，由调用方给友好提示）。
pub fn parse_datetime(s: &str, tz_min: i64) -> Option<i64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut text = s.to_string();
    let mut utc = false;
    if let Some(stripped) = text.strip_suffix('Z') {
        text = stripped.to_string();
        utc = true;
    }
    if let Some(stripped) = text.strip_suffix(" UTC") {
        text = stripped.to_string();
        utc = true;
    }
    let (date_part, time_part) = match text.find(['T', ' ']) {
        Some(i) => (text[..i].to_string(), text[i + 1..].to_string()),
        None => (text.clone(), String::new()),
    };
    let digits: String = date_part.chars().filter(|c| c.is_ascii_digit()).collect();
    let (year, month, day) = if date_part.contains('-') || date_part.contains('/') {
        let sep = if date_part.contains('-') { '-' } else { '/' };
        let mut it = date_part.split(sep);
        (
            it.next()?.trim().parse::<i64>().ok()?,
            it.next()?.trim().parse::<u32>().ok()?,
            it.next()?.trim().parse::<u32>().ok()?,
        )
    } else if digits.len() == 8 || digits.len() == 14 {
        (
            digits[0..4].parse().ok()?,
            digits[4..6].parse().ok()?,
            digits[6..8].parse().ok()?,
        )
    } else {
        return None;
    };
    if !(1..=12).contains(&month) || day == 0 || day > days_in_month(year, month) {
        return None;
    }
    let mut hour = 0;
    let mut minute = 0;
    let mut second = 0;
    if !time_part.is_empty() {
        let parts: Vec<&str> = time_part.trim().split(':').collect();
        if parts.len() >= 2 {
            hour = parts[0].trim().parse().ok()?;
            minute = parts[1].trim().parse().ok()?;
            if parts.len() >= 3 {
                let sec = parts[2].trim();
                second = sec.split('.').next().unwrap_or(sec).parse().ok()?;
            }
        }
    } else if digits.len() == 14 {
        hour = digits[8..10].parse().ok()?;
        minute = digits[10..12].parse().ok()?;
        second = digits[12..14].parse().ok()?;
    }
    if hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    Some(ms_from_local(
        year,
        month,
        day,
        hour,
        minute,
        second,
        if utc { 0 } else { tz_min },
    ))
}

pub fn iso8601(ms: i64) -> String {
    let p = local_parts(ms, 0);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        p.year, p.month, p.day, p.hour, p.minute, p.second
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_roundtrip() {
        for z in [-1000, 0, 1, 19_000, 20_000] {
            let (y, m, d) = civil_from_days(z);
            assert_eq!(days_from_civil(y, m, d), z);
        }
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(weekday_from_days(0), 4);
    }

    #[test]
    fn parses_common_forms() {
        let ms = parse_datetime("2024-01-01 12:30:00", 480).unwrap();
        assert_eq!(ms, 1_704_083_400_000);
        assert_eq!(format_ms(ms, 480), "2024-01-01 12:30:00 周一");
        assert_eq!(parse_datetime("2024-01-01T04:30:00Z", 480), Some(ms));
        assert!(parse_datetime("2024/1/1", 480).is_some());
        assert!(parse_datetime("2024-02-30", 480).is_none());
        assert!(parse_datetime("20240101123000", 480).is_some());
    }

    #[test]
    fn relative_is_human() {
        assert_eq!(relative(0, 3000), "3 秒前");
        assert_eq!(relative(10_000, 0), "10 秒后");
    }
}
