//! Cron 表达式：中文释义 + 未来 N 次执行时间。
//!
//! 支持 5/6/7 段（Spring 与 Quartz 方言）、`*` `?` 逗号、范围、步长、
//! 月份/星期英文缩写，以及 `L` `L-n` `W` `#` 的常见用法。`W` 按普通日期近似。

use crate::tools::datetime::{
    WEEK_CN, civil_from_days, days_in_month, format_parts, local_parts, ms_from_local,
    weekday_from_days,
};
use crate::tools::meta::{Group, OptionSpec, ToolMeta};
use crate::tools::{RunCtx, Tool, ToolResult};

pub struct CronExplain;

const MONTH_NAMES: [(&str, u32); 12] = [
    ("JAN", 1),
    ("FEB", 2),
    ("MAR", 3),
    ("APR", 4),
    ("MAY", 5),
    ("JUN", 6),
    ("JUL", 7),
    ("AUG", 8),
    ("SEP", 9),
    ("OCT", 10),
    ("NOV", 11),
    ("DEC", 12),
];

const DOW_NAMES: [(&str, u32); 7] = [
    ("SUN", 0),
    ("MON", 1),
    ("TUE", 2),
    ("WED", 3),
    ("THU", 4),
    ("FRI", 5),
    ("SAT", 6),
];

const MAX_DAYS: i64 = 366 * 6;

impl Tool for CronExplain {
    fn meta(&self) -> ToolMeta {
        ToolMeta::new(
            "cron",
            "Cron 中文释义",
            Group::Time,
            "把 Cron 表达式翻译成中文，并列出未来几次执行时间",
            "0 9 * * 1-5\n0 */5 * * * ?",
            true,
            "text",
        )
        .with_options(vec![
            OptionSpec::select(
                "cronDialect",
                "方言",
                "spring",
                &[
                    ("spring", "Spring（0/7 = 周日）"),
                    ("quartz", "Quartz（1 = 周日）"),
                ],
                "星期字段的起始值不同",
            ),
            OptionSpec::number(
                "nextCount",
                "列出几次执行",
                5.0,
                1.0,
                20.0,
                "按当前时间往后推",
            ),
        ])
    }

    fn run(&self, input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
        explain(input, ctx)
    }
}

#[derive(Default, Clone, Debug)]
struct Flags {
    any: bool,
    last: bool,
    last_offset: u32,
    nth: Option<(u32, u32)>,
}

#[derive(Clone, Debug)]
struct Spec {
    secs: Vec<u32>,
    mins: Vec<u32>,
    hours: Vec<u32>,
    doms: Vec<u32>,
    months: Vec<u32>,
    dows: Vec<u32>,
    years: Option<Vec<u32>>,
    dom_any: bool,
    dow_any: bool,
    dom_flags: Flags,
    dow_flags: Flags,
}

fn parse_value(s: &str, names: &[(&str, u32)], dow: bool, dialect: &str) -> Result<u32, String> {
    if let Ok(n) = s.parse::<u32>() {
        if dow {
            if n > 7 {
                return Err(format!("星期超出范围：{s}"));
            }
            return Ok(if dialect == "quartz" {
                (n + 6) % 7
            } else {
                n % 7
            });
        }
        return Ok(n);
    }
    for (name, v) in names {
        if s.eq_ignore_ascii_case(name) {
            return Ok(*v);
        }
    }
    Err(format!("看不懂的字段值：{s}"))
}

fn parse_field(
    raw: &str,
    min: u32,
    max: u32,
    names: &[(&str, u32)],
    dow: bool,
    dialect: &str,
) -> Result<(Vec<u32>, Flags), String> {
    let upper = raw.trim().to_ascii_uppercase();
    let mut flags = Flags::default();
    if upper.is_empty() {
        return Err("字段不能为空".to_string());
    }
    if upper == "*" || upper == "?" {
        flags.any = true;
        return Ok(((min..=max).collect(), flags));
    }
    let mut vals: Vec<u32> = Vec::new();
    for part in upper.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if part == "L" {
            flags.last = true;
            vals.push(max);
            continue;
        }
        if let Some(rest) = part.strip_prefix("L-") {
            flags.last = true;
            flags.last_offset = rest.parse().map_err(|_| format!("看不懂的 L-{rest}"))?;
            vals.push(max);
            continue;
        }
        if let Some(rest) = part.strip_suffix('W') {
            vals.push(parse_value(rest, names, dow, dialect)?);
            continue;
        }
        if let Some((base, n)) = part.split_once('#') {
            let day = parse_value(base, names, dow, dialect)?;
            let nth: u32 = n.parse().map_err(|_| format!("看不懂的 #{n}"))?;
            if !(1..=5).contains(&nth) {
                return Err("第几个星期几只能是 1–5".to_string());
            }
            flags.nth = Some((day, nth));
            vals.push(day);
            continue;
        }
        let (base, step) = match part.split_once('/') {
            Some((b, s)) => (
                b,
                s.parse::<u32>().map_err(|_| format!("看不懂的步长 {s}"))?,
            ),
            None => (part, 1),
        };
        if step == 0 {
            return Err("步长不能是 0".to_string());
        }
        let (lo, hi) = if base == "*" {
            (min, max)
        } else if let Some((a, b)) = base.split_once('-') {
            (
                parse_value(a, names, dow, dialect)?,
                parse_value(b, names, dow, dialect)?,
            )
        } else {
            let v = parse_value(base, names, dow, dialect)?;
            if step == 1 { (v, v) } else { (v, max) }
        };
        if lo > hi || lo < min || hi > max {
            return Err(format!("{part} 超出范围 {min}–{max}"));
        }
        let mut v = lo;
        while v <= hi {
            vals.push(v);
            v += step;
        }
    }
    vals.sort_unstable();
    vals.dedup();
    Ok((vals, flags))
}

fn parse_spec(input: &str, dialect: &str) -> Result<Spec, String> {
    let cols: Vec<&str> = input.split_whitespace().collect();
    if !(5..=7).contains(&cols.len()) {
        return Err(format!(
            "Cron 需要 5–7 段，当前 {} 段；5 段是分 时 日 月 周，6 段多了秒，7 段再多一年",
            cols.len()
        ));
    }
    let (sec, min, hour, dom, mon, dow, year) = match cols.len() {
        5 => ("0", cols[0], cols[1], cols[2], cols[3], cols[4], "*"),
        6 => (cols[0], cols[1], cols[2], cols[3], cols[4], cols[5], "*"),
        _ => (
            cols[0], cols[1], cols[2], cols[3], cols[4], cols[5], cols[6],
        ),
    };
    let (secs, _) = parse_field(sec, 0, 59, &[], false, dialect)?;
    let (mins, _) = parse_field(min, 0, 59, &[], false, dialect)?;
    let (hours, _) = parse_field(hour, 0, 23, &[], false, dialect)?;
    let (doms, dom_flags) = parse_field(dom, 1, 31, &[], false, dialect)?;
    let (months, _) = parse_field(mon, 1, 12, &MONTH_NAMES, false, dialect)?;
    let (dows, dow_flags) = parse_field(dow, 0, 7, &DOW_NAMES, true, dialect)?;
    let years = if year.trim() == "*" || year.trim() == "?" {
        None
    } else {
        Some(parse_field(year, 1970, 2099, &[], false, dialect)?.0)
    };
    Ok(Spec {
        dom_any: dom_flags.any,
        dow_any: dow_flags.any,
        dom_flags,
        dow_flags,
        secs,
        mins,
        hours,
        doms,
        months,
        dows,
        years,
    })
}

fn dom_matches(spec: &Spec, year: i64, month: u32, day: u32) -> bool {
    if spec.dom_flags.last {
        let last = days_in_month(year, month);
        return day + spec.dom_flags.last_offset == last;
    }
    spec.doms.contains(&day)
}

fn dow_matches(spec: &Spec, year: i64, month: u32, day: u32, weekday: u32) -> bool {
    if let Some((dow, nth)) = spec.dow_flags.nth {
        return weekday == dow && (day - 1) / 7 + 1 == nth;
    }
    if spec.dow_flags.last {
        return weekday == spec.dows[0] && day + 7 > days_in_month(year, month);
    }
    spec.dows.contains(&weekday)
}

fn date_matches(spec: &Spec, year: i64, month: u32, day: u32, weekday: u32) -> bool {
    if !spec.months.contains(&month) {
        return false;
    }
    if let Some(years) = &spec.years
        && !years.contains(&(year as u32))
    {
        return false;
    }
    let dom_ok = dom_matches(spec, year, month, day);
    let dow_ok = dow_matches(spec, year, month, day, weekday);
    match (spec.dom_any, spec.dow_any) {
        (true, true) => true,
        (true, false) => dow_ok,
        (false, true) => dom_ok,
        (false, false) => dom_ok || dow_ok,
    }
}

fn next_runs(spec: &Spec, from_ms: i64, tz: i64, count: usize) -> Vec<String> {
    let mut out = Vec::new();
    let start = local_parts(from_ms + 1000, tz).day_index;
    let mut budget: u64 = 2_000_000;
    for day in start..start.saturating_add(MAX_DAYS) {
        let (year, month, d) = civil_from_days(day);
        let weekday = weekday_from_days(day);
        if date_matches(spec, year, month, d, weekday) {
            'outer: for h in &spec.hours {
                for mi in &spec.mins {
                    for s in &spec.secs {
                        if budget == 0 {
                            return out;
                        }
                        budget -= 1;
                        let ms = ms_from_local(year, month, d, *h, *mi, *s, tz);
                        if ms > from_ms {
                            out.push(format_parts(&local_parts(ms, tz)));
                            if out.len() >= count {
                                break 'outer;
                            }
                        }
                    }
                }
            }
        }
        if out.len() >= count || budget == 0 {
            break;
        }
    }
    out
}

fn step_of(vals: &[u32]) -> Option<u32> {
    if vals.len() < 2 {
        return None;
    }
    let step = vals[1] - vals[0];
    if step == 0 {
        return None;
    }
    vals.windows(2).all(|w| w[1] - w[0] == step).then_some(step)
}

fn join_nums(vals: &[u32]) -> String {
    vals.iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join("、")
}

fn weekday_phrase(dows: &[u32]) -> String {
    if dows.is_empty() {
        return String::new();
    }
    let sorted = dows.windows(2).all(|w| w[1] == w[0] + 1);
    if dows.len() > 2 && sorted {
        return format!(
            "{}至{}",
            WEEK_CN[dows[0] as usize],
            WEEK_CN[dows[dows.len() - 1] as usize]
        );
    }
    dows.iter()
        .map(|d| WEEK_CN[*d as usize])
        .collect::<Vec<_>>()
        .join("、")
}

fn describe(spec: &Spec) -> Vec<String> {
    let mut lines = Vec::new();
    let time = if spec.hours.len() == 1 && spec.mins.len() == 1 {
        let h = spec.hours[0];
        let m = spec.mins[0];
        if spec.secs.len() == 1 && spec.secs[0] != 0 {
            format!("{:02}:{:02}:{:02}", h, m, spec.secs[0])
        } else {
            format!("{:02}:{:02}", h, m)
        }
    } else if let Some(step) = step_of(&spec.mins)
        && spec.mins.len() > 2
        && spec.hours.len() == 24
    {
        format!("每 {step} 分钟")
    } else if let Some(step) = step_of(&spec.hours)
        && spec.hours.len() > 1
        && spec.mins.len() == 1
        && spec.mins[0] == 0
    {
        format!("每 {step} 小时")
    } else if spec.mins.len() == 1 {
        format!("每小时第 {:02} 分", spec.mins[0])
    } else {
        format!("第 {} 分", join_nums(&spec.mins))
    };

    let mut date = String::new();
    if let Some(y) = &spec.years {
        if y.len() == 1 {
            date.push_str(&format!("{} 年 ", y[0]));
        } else {
            date.push_str(&format!("{} 年 ", join_nums(y)));
        }
    }
    if spec.months.len() < 12 {
        if date.is_empty() {
            date.push_str("每年 ");
        }
        date.push_str(&format!("{} 月 ", join_nums(&spec.months)));
    }
    if !spec.dom_any {
        if date.is_empty() {
            date.push_str("每月 ");
        }
        if spec.dom_flags.last {
            date.push_str("最后一天 ");
        } else {
            date.push_str(&format!("{} 日 ", join_nums(&spec.doms)));
        }
    }
    if !spec.dow_any {
        if date.is_empty() {
            date.push_str(&format!("每{} ", weekday_phrase(&spec.dows)));
        } else {
            date.push_str(&format!("{} ", weekday_phrase(&spec.dows)));
        }
    }
    if date.is_empty() {
        date.push_str("每天 ");
    }

    if time.starts_with("每 ") && date.trim() == "每天" {
        lines.push(format!("{time}执行"));
    } else {
        lines.push(format!("{date}{time} 执行"));
    }
    if spec.dow_flags.nth.is_some() {
        lines.push("按「第 N 个星期几」匹配".to_string());
    }
    if spec.dom_flags.last {
        lines.push("L 表示当月最后一天".to_string());
    }
    lines
}

fn dump(spec: &Spec) -> String {
    let row = |name: &str, vals: &[u32], any: bool| {
        if any {
            format!("{name}  *")
        } else if vals.len() > 6 {
            format!("{name}  {} …（{} 个值）", join_nums(&vals[..6]), vals.len())
        } else {
            format!("{name}  {}", join_nums(vals))
        }
    };
    [
        row("秒", &spec.secs, false),
        row("分", &spec.mins, false),
        row("时", &spec.hours, false),
        row("日", &spec.doms, spec.dom_any),
        row("月", &spec.months, spec.months.len() == 12),
        row("周", &spec.dows, spec.dow_any),
        match &spec.years {
            Some(y) => format!("年  {}", join_nums(y)),
            None => "年  *".to_string(),
        },
    ]
    .join("\n")
}

fn explain(input: &str, ctx: &RunCtx) -> Result<ToolResult, String> {
    let expr = input
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    if expr.is_empty() {
        return Err("粘一个 Cron 表达式，例如 0 9 * * 1-5".to_string());
    }
    let dialect = ctx.str("cronDialect", "spring");
    let spec = parse_spec(expr, dialect)?;
    let count = ctx.usize("nextCount", 5, 1, 20);
    let tz = ctx.i64("timezone", 480);
    let desc = describe(&spec).join("；");
    let runs = next_runs(&spec, ctx.now_ms, tz, count);
    let mut r = ToolResult::new().section("中文释义", "text", desc).section(
        "解析结果",
        "text",
        dump(&spec),
    );
    if runs.is_empty() {
        r = r.section("未来执行", "text", "未来 6 年内没有匹配时间，请检查表达式");
    } else {
        r = r.section("未来执行", "text", runs.join("\n"));
    }
    Ok(r.note(format!(
        "方言：{}；当前时间：{}",
        if dialect == "quartz" {
            "Quartz"
        } else {
            "Spring"
        },
        format_parts(&local_parts(ctx.now_ms, tz))
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn ctx(opt: Value) -> RunCtx<'static> {
        let leaked: &'static Value = Box::leak(Box::new(opt));
        RunCtx {
            options: leaked,
            now_ms: 1_704_083_400_000,
        }
    }

    #[test]
    fn parses_every_five_minutes() {
        let spec = parse_spec("*/5 * * * *", "spring").unwrap();
        assert_eq!(
            spec.mins,
            vec![0, 5, 10, 15, 20, 25, 30, 35, 40, 45, 50, 55]
        );
        assert_eq!(spec.hours.len(), 24);
    }

    #[test]
    fn weekday_range_is_compressed_in_description() {
        let spec = parse_spec("0 9 * * 1-5", "spring").unwrap();
        let d = describe(&spec).join("；");
        assert!(d.contains("周一至周五"));
        assert!(d.contains("09:00"));
    }

    #[test]
    fn quartz_weekday_mapping() {
        let spec = parse_spec("0 0 12 ? * 2", "quartz").unwrap();
        assert_eq!(spec.dows, vec![1]);
    }

    #[test]
    fn next_runs_are_sorted_and_limited() {
        let spec = parse_spec("0 9 * * 1-5", "spring").unwrap();
        let runs = next_runs(&spec, 1_704_083_400_000, 480, 3);
        assert_eq!(runs.len(), 3);
        assert!(runs[0] < runs[1]);
    }

    #[test]
    fn last_day_and_nth_weekday() {
        let spec = parse_spec("0 0 L * ?", "spring").unwrap();
        assert!(spec.dom_flags.last);
        let spec = parse_spec("0 0 ? * 1#2", "spring").unwrap();
        assert_eq!(spec.dow_flags.nth, Some((1, 2)));
    }

    #[test]
    fn invalid_field_reports_error() {
        assert!(parse_spec("0 9 * * 9", "spring").is_err());
        assert!(parse_spec("0 9 *", "spring").is_err());
    }

    #[test]
    fn explain_has_sections() {
        let r = CronExplain
            .run("0 9 * * 1-5", &ctx(json!({"nextCount": 2})))
            .unwrap();
        assert_eq!(r.sections.len(), 3);
        assert!(r.sections[0].text.contains("09:00"));
    }
}
