//! `ActiveSupport::TimeWithZone` answers most methods through `method_missing` on its local
//! `Time` and wraps a `Time` result back into the zone (`wrap_with_time_zone`).

use crate::types::{Sym, Type};

pub const TIME_WITH_ZONE_CLASS: &str = "ActiveSupport::TimeWithZone";

/// Methods `TimeWithZone` answers itself (or through the calendar extensions), without
/// forwarding to `Time`.
pub fn own_method_return(method_name: &str) -> Option<Type> {
    let class = |name: &str| Type::Class(Sym::new(name));
    Some(match method_name {
        "utc" | "getutc" | "to_time" | "localtime" => class("Time"),
        "to_date" => class("Date"),
        "to_datetime" => class("DateTime"),
        "strftime" | "iso8601" | "xmlschema" | "rfc3339" | "rfc2822" | "to_s" | "inspect"
        | "zone" | "httpdate" | "as_json" => Type::String,
        "to_i" | "tv_sec" | "year" | "month" | "mon" | "day" | "mday" | "hour" | "min" | "sec"
        | "wday" | "yday" | "usec" | "nsec" | "utc_offset" => Type::Integer,
        "to_f" => Type::Float,
        "dst?" | "utc?" | "gmt?" | "blank?" | "present?" | "<" | "<=" | ">" | ">=" | "=="
        | "between?" | "eql?" => Type::Bool,
        "time" | "period" | "comparable_time" => Type::Untyped,
        "<=>" => Type::Integer.union_with(Type::Nil),
        _ => return calendar_method_return(method_name, &class(TIME_WITH_ZONE_CLASS)),
    })
}

/// ActiveSupport calendar extensions shared by `Time`, `Date`, `DateTime` and `TimeWithZone`.
pub fn calendar_method_return(method_name: &str, receiver: &Type) -> Option<Type> {
    match method_name {
        "beginning_of_day"
        | "end_of_day"
        | "beginning_of_week"
        | "end_of_week"
        | "beginning_of_month"
        | "end_of_month"
        | "beginning_of_quarter"
        | "end_of_quarter"
        | "beginning_of_year"
        | "end_of_year"
        | "beginning_of_hour"
        | "end_of_hour"
        | "beginning_of_minute"
        | "end_of_minute"
        | "midnight"
        | "midday"
        | "noon"
        | "at_midnight"
        | "at_noon"
        | "yesterday"
        | "tomorrow"
        | "next_day"
        | "prev_day"
        | "next_week"
        | "prev_week"
        | "next_month"
        | "prev_month"
        | "next_year"
        | "prev_year"
        | "weeks_ago"
        | "weeks_since"
        | "months_ago"
        | "months_since"
        | "years_ago"
        | "years_since"
        | "days_ago"
        | "days_since"
        | "advance"
        | "change" => Some(receiver.clone()),
        "ago" | "since" | "in" => Some(receiver.clone()),
        "future?" | "past?" | "today?" | "tomorrow?" | "yesterday?" | "on_weekday?"
        | "on_weekend?" => Some(Type::Bool),
        "in_time_zone" => Some(Type::Class(Sym::new(TIME_WITH_ZONE_CLASS))),
        "to_fs" | "to_formatted_s" => Some(Type::String),
        "seconds_since_midnight" => Some(Type::Float),
        "all_day" | "all_week" | "all_month" | "all_year" => Some(Type::Untyped),
        _ => None,
    }
}

/// The receiver a method missing on `class_name` is forwarded to.
pub fn method_missing_target(class_name: &str, method_name: &str) -> Option<Type> {
    // `-` / `+` are TimeWithZone's own methods with argument-dependent overloads.
    (class_name == TIME_WITH_ZONE_CLASS && !matches!(method_name, "+" | "-"))
        .then(|| Type::Class(Sym::new("Time")))
}

/// A `Time` in a forwarded result comes back as a `TimeWithZone`.
pub fn wrap_forwarded_result(ty: Type) -> Type {
    match ty {
        Type::Class(name) if name.as_str() == "Time" => Type::Class(Sym::new(TIME_WITH_ZONE_CLASS)),
        Type::Union(parts) => {
            Type::from_type_vec(parts.into_iter().map(wrap_forwarded_result).collect())
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forwards_to_time_except_own_arithmetic() {
        assert!(method_missing_target(TIME_WITH_ZONE_CLASS, "round").is_some());
        assert!(method_missing_target(TIME_WITH_ZONE_CLASS, "-").is_none());
        assert!(method_missing_target("Time", "round").is_none());
        assert_eq!(
            wrap_forwarded_result(Type::Union(vec![Type::Class(Sym::new("Time")), Type::Nil])),
            Type::from_type_vec(vec![Type::Class(Sym::new(TIME_WITH_ZONE_CLASS)), Type::Nil])
        );
    }
}
