//! Consent-exported onboarding observations and deterministic aggregate summaries.
//! This module performs no collection and no I/O; callers must obtain explicit consent.

use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct LessonObservation {
    pub lesson_id: u8,
    pub completed: bool,
    pub time_to_threshold_ms: u64,
    pub retry_count: u32,
    pub hint_open_count: u32,
    pub causal_rules_triggered: Vec<String>,
    pub first_burn_direction_on_phasing: Option<String>,
    pub schema5_mass_delta_kg: Option<f64>,
    pub prediction_error_before_redesign_kg: Option<f64>,
    pub prediction_error_final_kg: Option<f64>,
    pub drop_off_step: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlaytestSessionLog {
    pub schema: u8,
    pub session_id: String,
    pub started_at_ms: u64,
    pub observations: Vec<LessonObservation>,
}

#[derive(Default)]
struct Aggregate {
    sessions: usize,
    completions: usize,
    times: Vec<u64>,
    rules: BTreeMap<String, usize>,
    improvements: Vec<f64>,
}

/// Parse the deliberately small, versioned JSON shape exported by the browser.
/// Unknown fields are ignored; required fields fail closed.
pub fn parse_session_json(json: &str) -> Result<PlaytestSessionLog, &'static str> {
    let schema_number = integer(json, "schema")?;
    if schema_number != 1 {
        return Err("unsupported playtest schema");
    }
    let schema = 1;
    let session_id = string(json, "session_id")?;
    let started_at_ms = integer(json, "started_at_ms")?;
    let array = value_after(json, "observations")?;
    let open = array.find('[').ok_or("observations is not an array")?;
    let end = matching(array, open, '[', ']').ok_or("unterminated observations")?;
    let mut observations = Vec::new();
    let mut cursor = open + 1;
    while let Some(relative) = array[cursor..end].find('{') {
        let start = cursor + relative;
        let stop = matching(array, start, '{', '}').ok_or("unterminated observation")?;
        let object = &array[start..=stop];
        observations.push(LessonObservation {
            lesson_id: u8::try_from(integer(object, "lesson_id")?)
                .map_err(|_| "lesson id out of range")?,
            completed: boolean(object, "completed")?,
            time_to_threshold_ms: integer(object, "time_to_threshold_ms")?,
            retry_count: u32::try_from(integer(object, "retry_count")?)
                .map_err(|_| "retry count out of range")?,
            hint_open_count: u32::try_from(integer(object, "hint_open_count")?)
                .map_err(|_| "hint count out of range")?,
            causal_rules_triggered: string_array(object, "causal_rules_triggered")?,
            first_burn_direction_on_phasing: optional_string(
                object,
                "first_burn_direction_on_phasing",
            )?,
            schema5_mass_delta_kg: optional_number(object, "schema5_mass_delta_kg")?,
            prediction_error_before_redesign_kg: optional_number(
                object,
                "prediction_error_before_redesign_kg",
            )?,
            prediction_error_final_kg: optional_number(object, "prediction_error_final_kg")?,
            drop_off_step: optional_string(object, "drop_off_step")?,
        });
        cursor = stop + 1;
    }
    Ok(PlaytestSessionLog {
        schema,
        session_id,
        started_at_ms,
        observations,
    })
}

fn value_after<'a>(json: &'a str, key: &str) -> Result<&'a str, &'static str> {
    let needle = format!("\"{key}\"");
    let rest = json.split_once(&needle).ok_or("missing field")?.1;
    Ok(rest
        .split_once(':')
        .ok_or("field has no value")?
        .1
        .trim_start())
}
fn number(json: &str, key: &str) -> Result<f64, &'static str> {
    optional_number(json, key)?.ok_or("number is null")
}
fn integer(json: &str, key: &str) -> Result<u64, &'static str> {
    let value = number(json, key)?;
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > u64::MAX as f64 {
        return Err("invalid non-negative integer");
    }
    Ok(value as u64)
}
fn optional_number(json: &str, key: &str) -> Result<Option<f64>, &'static str> {
    let rest = value_after(json, key)?;
    if rest.starts_with("null") {
        return Ok(None);
    }
    let token = rest
        .split(|c: char| c == ',' || c == '}' || c.is_whitespace())
        .next()
        .unwrap_or("");
    let value: f64 = token.parse().map_err(|_| "invalid number")?;
    if !value.is_finite() {
        return Err("non-finite number");
    }
    Ok(Some(value))
}
fn boolean(json: &str, key: &str) -> Result<bool, &'static str> {
    let rest = value_after(json, key)?;
    if rest.starts_with("true") {
        Ok(true)
    } else if rest.starts_with("false") {
        Ok(false)
    } else {
        Err("invalid boolean")
    }
}
fn string(json: &str, key: &str) -> Result<String, &'static str> {
    optional_string(json, key)?.ok_or("string is null")
}
fn optional_string(json: &str, key: &str) -> Result<Option<String>, &'static str> {
    let rest = value_after(json, key)?;
    if rest.starts_with("null") {
        return Ok(None);
    }
    if !rest.starts_with('"') {
        return Err("invalid string");
    }
    let mut escaped = false;
    let mut out = String::new();
    for character in rest[1..].chars() {
        if escaped {
            out.push(character);
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Ok(Some(out));
        } else {
            out.push(character);
        }
    }
    Err("unterminated string")
}
fn string_array(json: &str, key: &str) -> Result<Vec<String>, &'static str> {
    let rest = value_after(json, key)?;
    let open = rest.find('[').ok_or("invalid string array")?;
    let end = matching(rest, open, '[', ']').ok_or("unterminated string array")?;
    let mut values = Vec::new();
    let mut cursor = open + 1;
    while let Some(relative) = rest[cursor..end].find('"') {
        cursor += relative;
        let value = optional_string(&format!("\"v\":{}", &rest[cursor..end]), "v")?
            .ok_or("null array string")?;
        cursor += value.len() + 2;
        values.push(value);
    }
    Ok(values)
}
fn matching(text: &str, start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for (offset, character) in text[start..].char_indices() {
        if quoted {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                quoted = false;
            }
            continue;
        }
        if character == '"' {
            quoted = true;
        } else if character == open {
            depth += 1;
        } else if character == close {
            depth -= 1;
            if depth == 0 {
                return Some(start + offset);
            }
        }
    }
    None
}

/// Produce a stable markdown bottleneck table from consent-exported sessions.
pub fn summarize_markdown(logs: &[PlaytestSessionLog]) -> String {
    let mut lessons: BTreeMap<u8, Aggregate> = BTreeMap::new();
    for log in logs {
        for observation in &log.observations {
            let aggregate = lessons.entry(observation.lesson_id).or_default();
            aggregate.sessions += 1;
            if observation.completed {
                aggregate.completions += 1;
                aggregate.times.push(observation.time_to_threshold_ms);
            }
            for rule in &observation.causal_rules_triggered {
                *aggregate.rules.entry(rule.clone()).or_default() += 1;
            }
            if let (Some(before), Some(after)) = (
                observation.prediction_error_before_redesign_kg,
                observation.prediction_error_final_kg,
            ) {
                if before.is_finite() && after.is_finite() {
                    aggregate.improvements.push(before - after);
                }
            }
        }
    }
    let mut out = String::from("# Odderon playtest bottlenecks\n\n| Lesson | Completion | Median threshold | Top causal rule | Schema-5 error improvement |\n|---:|---:|---:|---|---:|\n");
    for (lesson, mut aggregate) in lessons {
        aggregate.times.sort_unstable();
        let median = median_u64(&aggregate.times);
        let top_rule = aggregate
            .rules
            .into_iter()
            .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
            .map(|(rule, count)| format!("{rule} ({count})"))
            .unwrap_or_else(|| "—".into());
        let improvement = median_f64(&mut aggregate.improvements)
            .map(|value| format!("{value:.2} kg"))
            .unwrap_or_else(|| "—".into());
        let completion = if aggregate.sessions == 0 {
            0.0
        } else {
            aggregate.completions as f64 * 100.0 / aggregate.sessions as f64
        };
        out.push_str(&format!(
            "| {lesson} | {}/{} ({completion:.1}%) | {} ms | {top_rule} | {improvement} |\n",
            aggregate.completions, aggregate.sessions, median
        ));
    }
    out
}

fn median_u64(values: &[u64]) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        values[mid - 1].saturating_add(values[mid]) / 2
    } else {
        values[mid]
    }
}

fn median_f64(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oracle_playtest_consent_privacy_and_export_analyzer() {
        let json = r#"{"schema":1,"session_id":"local-a","started_at_ms":10,"observations":[{"lesson_id":7,"completed":true,"time_to_threshold_ms":12000,"retry_count":1,"hint_open_count":2,"causal_rules_triggered":["PropellantDeficit"],"first_burn_direction_on_phasing":null,"schema5_mass_delta_kg":-180.0,"prediction_error_before_redesign_kg":14.0,"prediction_error_final_kg":1.5,"drop_off_step":null}]}"#;
        let log = parse_session_json(json).unwrap();
        assert_eq!(log.session_id, "local-a");
        assert_eq!(log.observations[0].prediction_error_final_kg, Some(1.5));
        let summary = summarize_markdown(&[log]);
        assert!(summary.contains("| 7 | 1/1 (100.0%) | 12000 ms"));
        assert!(summary.contains("PropellantDeficit (1)"));
        assert!(summary.contains("12.50 kg"));
    }
}
