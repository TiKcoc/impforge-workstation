// SPDX-License-Identifier: Apache-2.0
//! ForgeCalendar -- AI-Powered Calendar with ICS/iCal Import
//!
//! A full calendar built from scratch that auto-digests events from external
//! calendars via ICS/iCal import (Google Calendar, Outlook, Apple Calendar,
//! any standard .ics source). AI features include schedule analysis, optimal
//! meeting time suggestions, and auto-generated agenda documents.
//!
//! Storage: `~/.impforge/calendar/` with `calendars.json` + per-calendar
//! event files. All data is local-first, no external service dependencies.
//!
//! This module is part of ImpForge Phase 3 (Office/Productivity tools).

use std::path::{Path, PathBuf};

use chrono::{Datelike, Duration, IsoWeek, NaiveDate, NaiveDateTime, Utc, Weekday};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Subdirectory under `~/.impforge/` that holds calendar data.
const CALENDAR_DIR: &str = "calendar";

/// Events subdirectory (one JSON file per calendar).
const EVENTS_DIR: &str = "events";

/// Registry of calendars.
const CALENDARS_FILE: &str = "calendars.json";

/// Ollama HTTP timeout for AI requests.
const AI_TIMEOUT_SECS: u64 = 120;

/// Maximum events to import from a single ICS file.
const MAX_ICS_EVENTS: usize = 5000;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Where an event originated from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventSource {
    /// Created locally in ImpForge.
    Local,
    /// Imported from Google Calendar.
    GoogleImport,
    /// Imported from Outlook / Office 365.
    OutlookImport,
    /// Imported from Apple Calendar.
    AppleImport,
    /// Imported from an ICS/iCal URL or file.
    IcsImport { url: String },
}

/// A single calendar event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    /// ISO 8601 datetime string.
    pub start: String,
    /// ISO 8601 datetime string.
    pub end: String,
    pub all_day: bool,
    pub location: Option<String>,
    /// Hex color for visual display (overrides calendar color if set).
    pub color: Option<String>,
    /// Which calendar this event belongs to.
    pub calendar_id: String,
    /// RRULE recurrence string (RFC 5545).
    pub recurrence: Option<String>,
    /// Minutes before the event to show a reminder.
    pub reminder_minutes: Option<u32>,
    /// Email addresses or names of attendees.
    pub attendees: Vec<String>,
    /// Where this event came from.
    pub source: EventSource,
    pub created_at: String,
    pub updated_at: String,
}

/// A calendar container that groups events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Calendar {
    pub id: String,
    pub name: String,
    /// Hex color for event display.
    pub color: String,
    /// "local", "google", "outlook", "apple", "ics"
    pub source_type: String,
    /// ICS URL for calendars that support auto-sync.
    pub source_url: Option<String>,
    /// Whether this calendar is visible in the UI.
    pub visible: bool,
    /// Whether to auto-sync from the source URL.
    pub auto_sync: bool,
    /// ISO 8601 timestamp of last sync.
    pub last_synced: Option<String>,
    pub created_at: String,
}

/// A day's agenda with optional AI summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayAgenda {
    pub date: String,
    pub events: Vec<CalendarEvent>,
    pub ai_summary: Option<String>,
}

/// Result of an ICS import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub calendar_id: String,
    pub calendar_name: String,
    pub imported_count: usize,
    pub skipped_count: usize,
    pub errors: Vec<String>,
}

/// A suggested free time slot from the AI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSlot {
    pub start: String,
    pub end: String,
    pub score: f64,
    pub reason: String,
}

/// Input struct for creating events from the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateEventInput {
    pub title: String,
    pub description: Option<String>,
    pub start: String,
    pub end: String,
    pub all_day: bool,
    pub location: Option<String>,
    pub color: Option<String>,
    pub calendar_id: String,
    pub recurrence: Option<String>,
    pub reminder_minutes: Option<u32>,
    pub attendees: Vec<String>,
}

/// Input struct for updating events from the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEventInput {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub all_day: Option<bool>,
    pub location: Option<Option<String>>,
    pub color: Option<Option<String>>,
    pub calendar_id: Option<String>,
    pub recurrence: Option<Option<String>>,
    pub reminder_minutes: Option<Option<u32>>,
    pub attendees: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Recurrence & Reminder Types (Enterprise additions)
// ---------------------------------------------------------------------------

/// Recurrence frequency for repeating events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl std::fmt::Display for Frequency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Frequency::Daily => write!(f, "DAILY"),
            Frequency::Weekly => write!(f, "WEEKLY"),
            Frequency::Monthly => write!(f, "MONTHLY"),
            Frequency::Yearly => write!(f, "YEARLY"),
        }
    }
}

/// Structured recurrence rule (RFC 5545 RRULE subset).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecurrenceRule {
    pub frequency: Frequency,
    /// Repeat every N periods (e.g. every 2 weeks).
    pub interval: u32,
    /// End date (ISO 8601). If `None`, repeats indefinitely or until `count`.
    pub until: Option<String>,
    /// Maximum number of occurrences.
    pub count: Option<u32>,
    /// Day-of-week codes for weekly recurrence: "MO","TU","WE","TH","FR","SA","SU".
    pub by_day: Vec<String>,
}

impl RecurrenceRule {
    /// Serialize to an RFC 5545 RRULE string.
    fn to_rrule_string(&self) -> String {
        let mut parts = vec![format!("FREQ={}", self.frequency)];
        if self.interval > 1 {
            parts.push(format!("INTERVAL={}", self.interval));
        }
        if !self.by_day.is_empty() {
            parts.push(format!("BYDAY={}", self.by_day.join(",")));
        }
        if let Some(ref until) = self.until {
            // Convert ISO date to ICS compact format
            let compact = until.replace('-', "").replace(':', "").replace('T', "T");
            parts.push(format!("UNTIL={compact}"));
        }
        if let Some(count) = self.count {
            parts.push(format!("COUNT={count}"));
        }
        parts.join(";")
    }

    /// Parse an RFC 5545 RRULE string into a structured rule.
    fn from_rrule_string(rrule: &str) -> Option<Self> {
        let mut frequency = None;
        let mut interval = 1u32;
        let mut until = None;
        let mut count = None;
        let mut by_day = Vec::new();

        for part in rrule.split(';') {
            let kv: Vec<&str> = part.splitn(2, '=').collect();
            if kv.len() != 2 {
                continue;
            }
            match kv[0] {
                "FREQ" => {
                    frequency = match kv[1] {
                        "DAILY" => Some(Frequency::Daily),
                        "WEEKLY" => Some(Frequency::Weekly),
                        "MONTHLY" => Some(Frequency::Monthly),
                        "YEARLY" => Some(Frequency::Yearly),
                        _ => None,
                    };
                }
                "INTERVAL" => {
                    interval = kv[1].parse().unwrap_or(1);
                }
                "UNTIL" => {
                    until = Some(parse_ics_datetime(kv[1]));
                }
                "COUNT" => {
                    count = kv[1].parse().ok();
                }
                "BYDAY" => {
                    by_day = kv[1].split(',').map(|s| s.trim().to_string()).collect();
                }
                _ => {}
            }
        }

        Some(RecurrenceRule {
            frequency: frequency?,
            interval,
            until,
            count,
            by_day,
        })
    }

    /// Expand occurrences of a base event within a date range.
    fn expand_occurrences(
        &self,
        base_start: &str,
        base_end: &str,
        range_start: &str,
        range_end: &str,
    ) -> Vec<(String, String)> {
        let Some(start_dt) = NaiveDate::parse_from_str(
            base_start.get(..10).unwrap_or(""),
            "%Y-%m-%d",
        )
        .ok()
        else {
            return Vec::new();
        };
        let Some(end_dt) = NaiveDate::parse_from_str(
            base_end.get(..10).unwrap_or(""),
            "%Y-%m-%d",
        )
        .ok()
        else {
            return Vec::new();
        };
        let event_duration = end_dt.signed_duration_since(start_dt);

        let range_s = NaiveDate::parse_from_str(
            range_start.get(..10).unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or(start_dt);
        let range_e = NaiveDate::parse_from_str(
            range_end.get(..10).unwrap_or(""),
            "%Y-%m-%d",
        )
        .unwrap_or(start_dt + Duration::days(365));

        let until_date = self
            .until
            .as_ref()
            .and_then(|u| NaiveDate::parse_from_str(u.get(..10).unwrap_or(""), "%Y-%m-%d").ok())
            .unwrap_or(range_e);

        let max_count = self.count.unwrap_or(500).min(500) as usize;
        let start_time_suffix = base_start.get(10..).unwrap_or("");
        let end_time_suffix = base_end.get(10..).unwrap_or("");

        let mut occurrences = Vec::new();
        let mut current = start_dt;
        let mut generated = 0usize;

        let valid_weekdays: Vec<Weekday> = self
            .by_day
            .iter()
            .filter_map(|d| match d.as_str() {
                "MO" => Some(Weekday::Mon),
                "TU" => Some(Weekday::Tue),
                "WE" => Some(Weekday::Wed),
                "TH" => Some(Weekday::Thu),
                "FR" => Some(Weekday::Fri),
                "SA" => Some(Weekday::Sat),
                "SU" => Some(Weekday::Sun),
                _ => None,
            })
            .collect();

        // Safety limit to prevent runaway loops
        let mut iterations = 0u32;
        let max_iterations = 10_000u32;

        while current <= until_date && current <= range_e && generated < max_count && iterations < max_iterations {
            iterations += 1;

            let day_matches = if valid_weekdays.is_empty() {
                true
            } else {
                valid_weekdays.contains(&current.weekday())
            };

            if day_matches && current >= range_s {
                let occ_end = current + event_duration;
                let occ_start_str =
                    format!("{}{start_time_suffix}", current.format("%Y-%m-%d"));
                let occ_end_str =
                    format!("{}{end_time_suffix}", occ_end.format("%Y-%m-%d"));
                occurrences.push((occ_start_str, occ_end_str));
            }

            if day_matches {
                generated += 1;
            }

            // Advance cursor based on frequency
            current = match self.frequency {
                Frequency::Daily => current + Duration::days(self.interval as i64),
                Frequency::Weekly => {
                    if valid_weekdays.is_empty() {
                        current + Duration::weeks(self.interval as i64)
                    } else {
                        // For BYDAY weekly, advance one day at a time within the week
                        current + Duration::days(1)
                    }
                }
                Frequency::Monthly => {
                    let month = current.month0() + self.interval;
                    let year = current.year() + (month / 12) as i32;
                    let month = (month % 12) + 1;
                    let day = current.day().min(days_in_month(year, month));
                    NaiveDate::from_ymd_opt(year, month, day).unwrap_or(current + Duration::days(30))
                }
                Frequency::Yearly => {
                    let year = current.year() + self.interval as i32;
                    NaiveDate::from_ymd_opt(year, current.month(), current.day())
                        .unwrap_or(current + Duration::days(365))
                }
            };
        }

        occurrences
    }
}

/// Helper: days in a given month/year.
fn days_in_month(year: i32, month: u32) -> u32 {
    NaiveDate::from_ymd_opt(
        if month == 12 { year + 1 } else { year },
        if month == 12 { 1 } else { month + 1 },
        1,
    )
    .map(|d| d.pred_opt().map(|p| p.day()).unwrap_or(28))
    .unwrap_or(28)
}

/// An upcoming event reminder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventReminder {
    pub event_id: String,
    pub event_title: String,
    pub event_start: String,
    pub reminder_at: String,
    pub minutes_until: i64,
    pub calendar_name: String,
    pub location: Option<String>,
}

/// ISO week number information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekInfo {
    pub date: String,
    pub iso_week: u32,
    pub iso_year: i32,
    pub day_of_week: String,
    pub day_of_year: u32,
}

// ---------------------------------------------------------------------------
// Directory helpers
// ---------------------------------------------------------------------------

/// Resolve the calendar base directory, creating it if necessary.
fn calendar_base_dir() -> Result<PathBuf, ImpForgeError> {
    let base = dirs::home_dir()
        .ok_or_else(|| ImpForgeError::filesystem("HOME_DIR", "Cannot determine home directory"))?;
    let dir = base.join(".impforge").join(CALENDAR_DIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ImpForgeError::filesystem(
                "DIR_CREATE_FAILED",
                format!("Failed to create calendar directory: {e}"),
            )
        })?;
    }
    Ok(dir)
}

/// Resolve the events subdirectory.
fn events_dir() -> Result<PathBuf, ImpForgeError> {
    let dir = calendar_base_dir()?.join(EVENTS_DIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ImpForgeError::filesystem(
                "DIR_CREATE_FAILED",
                format!("Failed to create events directory: {e}"),
            )
        })?;
    }
    Ok(dir)
}

/// Path to the calendars registry file.
fn calendars_path() -> Result<PathBuf, ImpForgeError> {
    Ok(calendar_base_dir()?.join(CALENDARS_FILE))
}

/// Path to a calendar's events file.
fn events_path(calendar_id: &str) -> Result<PathBuf, ImpForgeError> {
    Ok(events_dir()?.join(format!("{calendar_id}.json")))
}

/// ISO-8601 timestamp for "now".
fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

// ---------------------------------------------------------------------------
// Persistence helpers
// ---------------------------------------------------------------------------

/// Read all calendars from disk.
fn read_calendars() -> Result<Vec<Calendar>, ImpForgeError> {
    let path = calendars_path()?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(&path).map_err(|e| {
        ImpForgeError::filesystem("CAL_READ_FAILED", format!("Cannot read calendars file: {e}"))
    })?;
    serde_json::from_str(&data).map_err(|e| {
        ImpForgeError::internal("CAL_PARSE_FAILED", format!("Corrupt calendars file: {e}"))
    })
}

/// Write all calendars to disk.
fn write_calendars(calendars: &[Calendar]) -> Result<(), ImpForgeError> {
    let path = calendars_path()?;
    let json = serde_json::to_string_pretty(calendars).map_err(|e| {
        ImpForgeError::internal("CAL_SERIALIZE", format!("Cannot serialize calendars: {e}"))
    })?;
    std::fs::write(&path, json).map_err(|e| {
        ImpForgeError::filesystem(
            "CAL_WRITE_FAILED",
            format!("Cannot write calendars file: {e}"),
        )
    })
}

/// Read events for a specific calendar.
fn read_events(calendar_id: &str) -> Result<Vec<CalendarEvent>, ImpForgeError> {
    let path = events_path(calendar_id)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(&path).map_err(|e| {
        ImpForgeError::filesystem(
            "EVT_READ_FAILED",
            format!("Cannot read events for calendar {calendar_id}: {e}"),
        )
    })?;
    serde_json::from_str(&data).map_err(|e| {
        ImpForgeError::internal(
            "EVT_PARSE_FAILED",
            format!("Corrupt events file for calendar {calendar_id}: {e}"),
        )
    })
}

/// Write events for a specific calendar.
fn write_events(calendar_id: &str, events: &[CalendarEvent]) -> Result<(), ImpForgeError> {
    let path = events_path(calendar_id)?;
    let json = serde_json::to_string_pretty(events).map_err(|e| {
        ImpForgeError::internal("EVT_SERIALIZE", format!("Cannot serialize events: {e}"))
    })?;
    std::fs::write(&path, json).map_err(|e| {
        ImpForgeError::filesystem(
            "EVT_WRITE_FAILED",
            format!("Cannot write events for calendar {calendar_id}: {e}"),
        )
    })
}

// ---------------------------------------------------------------------------
// ICS Parser (RFC 5545 — VCALENDAR / VEVENT)
// ---------------------------------------------------------------------------

/// Parse an ICS/iCal content string into calendar events.
///
/// Handles real-world exports from Google Calendar, Outlook, Apple Calendar.
/// Supports: SUMMARY, DTSTART, DTEND, DESCRIPTION, LOCATION, RRULE,
/// ATTENDEE, VALARM (for reminders). Handles both datetime formats:
///   - `20260315T140000Z` (UTC datetime)
///   - `20260315` (all-day date)
///   - `TZID=...` prefix on DTSTART/DTEND
fn parse_ics(content: &str, calendar_id: &str, source: EventSource) -> Vec<CalendarEvent> {
    let mut events = Vec::new();
    let now = now_iso();

    // Unfold continued lines per RFC 5545 Section 3.1:
    // A long line may be split into a continuation line by inserting a CRLF
    // immediately followed by a single whitespace character (space or tab).
    let unfolded = content
        .replace("\r\n ", "")
        .replace("\r\n\t", "")
        .replace("\n ", "")
        .replace("\n\t", "");

    let lines: Vec<&str> = unfolded.lines().collect();

    let mut in_event = false;
    let mut in_alarm = false;
    let mut summary = String::new();
    let mut description: Option<String> = None;
    let mut dtstart = String::new();
    let mut dtend = String::new();
    let mut location: Option<String> = None;
    let mut rrule: Option<String> = None;
    let mut attendees: Vec<String> = Vec::new();
    let mut reminder_minutes: Option<u32> = None;
    let mut all_day = false;

    for line in &lines {
        let trimmed = line.trim();

        if trimmed == "BEGIN:VEVENT" {
            in_event = true;
            in_alarm = false;
            summary.clear();
            description = None;
            dtstart.clear();
            dtend.clear();
            location = None;
            rrule = None;
            attendees.clear();
            reminder_minutes = None;
            all_day = false;
            continue;
        }

        if trimmed == "END:VEVENT" && in_event {
            in_event = false;
            in_alarm = false;

            if summary.is_empty() && dtstart.is_empty() {
                continue;
            }

            let start = parse_ics_datetime(&dtstart);
            let end = if dtend.is_empty() {
                // If no DTEND, assume 1 hour for timed or +1 day for all-day
                if all_day {
                    if let Some(d) = NaiveDate::parse_from_str(&start.get(..10).unwrap_or(""), "%Y-%m-%d").ok() {
                        let next = d + Duration::days(1);
                        next.format("%Y-%m-%d").to_string()
                    } else {
                        start.clone()
                    }
                } else {
                    start.clone()
                }
            } else {
                parse_ics_datetime(&dtend)
            };

            if events.len() >= MAX_ICS_EVENTS {
                break;
            }

            events.push(CalendarEvent {
                id: Uuid::new_v4().to_string(),
                title: unescape_ics_text(&summary),
                description: description.as_deref().map(unescape_ics_text),
                start,
                end,
                all_day,
                location: location.as_deref().map(unescape_ics_text),
                color: None,
                calendar_id: calendar_id.to_string(),
                recurrence: rrule.clone(),
                reminder_minutes,
                attendees: attendees.clone(),
                source: source.clone(),
                created_at: now.clone(),
                updated_at: now.clone(),
            });

            continue;
        }

        if trimmed == "BEGIN:VALARM" && in_event {
            in_alarm = true;
            continue;
        }
        if trimmed == "END:VALARM" && in_event {
            in_alarm = false;
            continue;
        }

        if !in_event {
            continue;
        }

        // Parse VALARM trigger for reminder
        if in_alarm && trimmed.starts_with("TRIGGER:") {
            let trigger = trimmed.trim_start_matches("TRIGGER:");
            reminder_minutes = parse_ics_duration_minutes(trigger);
            continue;
        }
        if in_alarm && trimmed.starts_with("TRIGGER;") {
            // TRIGGER;VALUE=DURATION:-PT15M
            if let Some(val) = trimmed.split(':').nth(1) {
                reminder_minutes = parse_ics_duration_minutes(val);
            }
            continue;
        }

        // Event properties
        if let Some(val) = strip_ics_prop(trimmed, "SUMMARY") {
            summary = val.to_string();
        } else if let Some(val) = strip_ics_prop(trimmed, "DESCRIPTION") {
            description = Some(val.to_string());
        } else if let Some(val) = strip_ics_prop(trimmed, "LOCATION") {
            location = Some(val.to_string());
        } else if let Some(val) = strip_ics_prop(trimmed, "RRULE") {
            rrule = Some(val.to_string());
        } else if trimmed.starts_with("DTSTART") {
            let val = extract_ics_value(trimmed);
            dtstart = val.to_string();
            // Detect all-day: date-only values are 8 chars (YYYYMMDD)
            all_day = val.len() == 8 && val.chars().all(|c| c.is_ascii_digit());
        } else if trimmed.starts_with("DTEND") {
            let val = extract_ics_value(trimmed);
            dtend = val.to_string();
        } else if trimmed.starts_with("ATTENDEE") {
            // ATTENDEE;CN=John Doe:mailto:john@example.com
            if let Some(mailto) = trimmed.split("mailto:").nth(1) {
                attendees.push(mailto.to_string());
            } else if let Some(cn) = extract_ics_param(trimmed, "CN") {
                attendees.push(cn.to_string());
            }
        }
    }

    events
}

/// Strip an ICS property name (with optional parameters) and return the value.
/// Handles both `PROP:value` and `PROP;PARAM=x:value` forms.
fn strip_ics_prop<'a>(line: &'a str, prop: &str) -> Option<&'a str> {
    if line.starts_with(prop) {
        let rest = &line[prop.len()..];
        if rest.starts_with(':') {
            return Some(&rest[1..]);
        }
        if rest.starts_with(';') {
            // Has parameters — find the colon after parameters
            if let Some(colon_pos) = rest.find(':') {
                return Some(&rest[colon_pos + 1..]);
            }
        }
    }
    None
}

/// Extract the value after the last colon in an ICS property line.
/// Handles `DTSTART;TZID=America/New_York:20260315T140000` and `DTSTART:20260315T140000Z`.
fn extract_ics_value(line: &str) -> &str {
    // For DTSTART/DTEND, the value is after the last colon
    line.rsplit(':').next().unwrap_or("")
}

/// Extract a named parameter from an ICS line.
/// e.g., `ATTENDEE;CN=John Doe:mailto:...` -> `Some("John Doe")`
fn extract_ics_param<'a>(line: &'a str, param: &str) -> Option<&'a str> {
    let search = format!("{param}=");
    if let Some(start) = line.find(&search) {
        let rest = &line[start + search.len()..];
        // Value ends at ; or :
        let end = rest.find([';', ':']).unwrap_or(rest.len());
        let val = &rest[..end];
        // Remove quotes if present
        Some(val.trim_matches('"'))
    } else {
        None
    }
}

/// Parse an ICS datetime string into ISO 8601 format.
///
/// Input formats:
///   - `20260315T140000Z`  -> `2026-03-15T14:00:00Z`
///   - `20260315T140000`   -> `2026-03-15T14:00:00`
///   - `20260315`          -> `2026-03-15`
fn parse_ics_datetime(raw: &str) -> String {
    let s = raw.trim();

    // All-day date: 8 digits
    if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) {
        if let Some(date) = NaiveDate::parse_from_str(s, "%Y%m%d").ok() {
            return date.format("%Y-%m-%d").to_string();
        }
        return s.to_string();
    }

    // DateTime with/without Z suffix
    let (datetime_part, suffix) = if s.ends_with('Z') {
        (&s[..s.len() - 1], "Z")
    } else {
        (s, "")
    };

    // 15 chars: YYYYMMDDTHHMMSS
    if datetime_part.len() == 15 && datetime_part.contains('T') {
        if let Some(dt) = NaiveDateTime::parse_from_str(datetime_part, "%Y%m%dT%H%M%S").ok() {
            return format!("{}{suffix}", dt.format("%Y-%m-%dT%H:%M:%S"));
        }
    }

    // Fallback: return as-is
    s.to_string()
}

/// Parse an ICS duration trigger into minutes.
/// e.g., `-PT15M` -> `Some(15)`, `-PT1H` -> `Some(60)`, `-P1D` -> `Some(1440)`
fn parse_ics_duration_minutes(raw: &str) -> Option<u32> {
    let s = raw.trim().trim_start_matches('-');
    let mut total: u32 = 0;
    let mut num_buf = String::new();

    for ch in s.chars() {
        if ch.is_ascii_digit() {
            num_buf.push(ch);
        } else {
            let n: u32 = num_buf.parse().unwrap_or(0);
            num_buf.clear();
            match ch {
                'W' => total += n * 7 * 24 * 60,
                'D' => total += n * 24 * 60,
                'H' => total += n * 60,
                'M' if s.contains('T') => total += n, // M after T = minutes
                'S' => total += n / 60, // round down
                _ => {}
            }
        }
    }

    if total > 0 { Some(total) } else { None }
}

/// Unescape ICS text encoding: `\\n` -> newline, `\\,` -> `,`, `\\;` -> `;`.
fn unescape_ics_text(s: &str) -> String {
    s.replace("\\n", "\n")
        .replace("\\N", "\n")
        .replace("\\,", ",")
        .replace("\\;", ";")
        .replace("\\\\", "\\")
}

/// Detect the source type based on ICS content heuristics.
fn detect_ics_source(content: &str) -> EventSource {
    let lower = content.to_lowercase();
    if lower.contains("google.com/calendar") || lower.contains("prodid:-//google") {
        EventSource::GoogleImport
    } else if lower.contains("microsoft") || lower.contains("prodid:-//microsoft") {
        EventSource::OutlookImport
    } else if lower.contains("apple") || lower.contains("prodid:-//apple") {
        EventSource::AppleImport
    } else {
        EventSource::IcsImport {
            url: String::new(),
        }
    }
}

/// Extract the calendar name from ICS X-WR-CALNAME property.
fn extract_ics_calname(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(val) = strip_ics_prop(trimmed, "X-WR-CALNAME") {
            let name = unescape_ics_text(val).trim().to_string();
            if !name.is_empty() {
                return Some(name);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Ollama AI helpers
// ---------------------------------------------------------------------------

/// Resolve the Ollama base URL from the environment.
fn resolve_ollama_url() -> String {
    std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://localhost:11434".to_string())
        .trim_end_matches('/')
        .to_string()
}

/// Call Ollama chat API and return the raw content string.
async fn ollama_chat(system_prompt: &str, user_message: &str, model: Option<&str>) -> Result<String, ImpForgeError> {
    let url = resolve_ollama_url();
    let model_name = model.unwrap_or("dolphin3:8b");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(AI_TIMEOUT_SECS))
        .build()
        .map_err(|e| {
            ImpForgeError::internal("HTTP_CLIENT", format!("Failed to build HTTP client: {e}"))
        })?;

    let response = client
        .post(format!("{url}/api/chat"))
        .json(&serde_json::json!({
            "model": model_name,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user",   "content": user_message },
            ],
            "stream": false,
        }))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                ImpForgeError::service(
                    "OLLAMA_UNREACHABLE",
                    "Cannot connect to Ollama for calendar AI features",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service("OLLAMA_TIMEOUT", "AI request timed out")
                    .with_suggestion("Try a simpler query or check Ollama status.")
            } else {
                ImpForgeError::service("OLLAMA_REQUEST_FAILED", format!("Ollama request failed: {e}"))
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(
            ImpForgeError::service("OLLAMA_HTTP_ERROR", format!("Ollama returned HTTP {status}"))
                .with_details(body)
                .with_suggestion("Check Ollama logs. The model may not be downloaded yet."),
        );
    }

    let body: serde_json::Value = response.json().await.map_err(|e| {
        ImpForgeError::service("OLLAMA_PARSE_ERROR", format!("Failed to parse Ollama response: {e}"))
    })?;

    let content = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    Ok(content)
}

// ---------------------------------------------------------------------------
// Tauri Commands — Calendar Management
// ---------------------------------------------------------------------------

/// List all calendars.
#[tauri::command]
pub async fn calendar_list() -> AppResult<Vec<Calendar>> {
    read_calendars()
}

/// Create a new local calendar.
#[tauri::command]
pub async fn calendar_create(name: String, color: String) -> AppResult<Calendar> {
    let mut calendars = read_calendars()?;

    let cal = Calendar {
        id: Uuid::new_v4().to_string(),
        name,
        color,
        source_type: "local".to_string(),
        source_url: None,
        visible: true,
        auto_sync: false,
        last_synced: None,
        created_at: now_iso(),
    };

    calendars.push(cal.clone());
    write_calendars(&calendars)?;

    // Create empty events file
    write_events(&cal.id, &[])?;

    Ok(cal)
}

/// Delete a calendar and its events.
#[tauri::command]
pub async fn calendar_delete(id: String) -> AppResult<()> {
    let mut calendars = read_calendars()?;
    let before = calendars.len();
    calendars.retain(|c| c.id != id);

    if calendars.len() == before {
        return Err(ImpForgeError::validation(
            "CAL_NOT_FOUND",
            format!("Calendar {id} not found"),
        ));
    }

    write_calendars(&calendars)?;

    // Remove events file
    let events_file = events_path(&id)?;
    if events_file.exists() {
        let _ = std::fs::remove_file(&events_file);
    }

    Ok(())
}

/// Import events from an ICS file or URL.
#[tauri::command]
pub async fn calendar_import_ics(url_or_path: String) -> AppResult<ImportResult> {
    let content = if url_or_path.starts_with("http://") || url_or_path.starts_with("https://") {
        // Fetch from URL
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| {
                ImpForgeError::internal("HTTP_CLIENT", format!("Failed to build HTTP client: {e}"))
            })?;

        let resp = client.get(&url_or_path).send().await.map_err(|e| {
            ImpForgeError::service(
                "ICS_FETCH_FAILED",
                format!("Failed to fetch ICS from URL: {e}"),
            )
            .with_suggestion("Check the URL and your internet connection.")
        })?;

        if !resp.status().is_success() {
            return Err(ImpForgeError::service(
                "ICS_HTTP_ERROR",
                format!("ICS URL returned HTTP {}", resp.status()),
            ));
        }

        resp.text().await.map_err(|e| {
            ImpForgeError::service("ICS_READ_FAILED", format!("Failed to read ICS response: {e}"))
        })?
    } else {
        // Read from local file
        let path = Path::new(&url_or_path);
        if !path.exists() {
            return Err(ImpForgeError::filesystem(
                "ICS_FILE_NOT_FOUND",
                format!("ICS file not found: {url_or_path}"),
            )
            .with_suggestion("Check the file path and try again."));
        }
        std::fs::read_to_string(path).map_err(|e| {
            ImpForgeError::filesystem(
                "ICS_FILE_READ",
                format!("Cannot read ICS file: {e}"),
            )
        })?
    };

    // Validate that it looks like an ICS file
    if !content.contains("BEGIN:VCALENDAR") {
        return Err(ImpForgeError::validation(
            "INVALID_ICS",
            "File does not appear to be a valid ICS/iCal file (missing BEGIN:VCALENDAR)",
        )
        .with_suggestion("Make sure you are importing a .ics file exported from Google Calendar, Outlook, or Apple Calendar."));
    }

    // Detect source and extract calendar name
    let source = if url_or_path.starts_with("http") {
        let mut detected = detect_ics_source(&content);
        if let EventSource::IcsImport { ref mut url } = detected {
            *url = url_or_path.clone();
        }
        detected
    } else {
        detect_ics_source(&content)
    };

    let cal_name = extract_ics_calname(&content)
        .unwrap_or_else(|| {
            // Derive name from filename or URL
            if url_or_path.starts_with("http") {
                "Imported Calendar".to_string()
            } else {
                Path::new(&url_or_path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Imported Calendar")
                    .to_string()
            }
        });

    let source_type = match &source {
        EventSource::GoogleImport => "google",
        EventSource::OutlookImport => "outlook",
        EventSource::AppleImport => "apple",
        _ => "ics",
    };

    // Create the calendar
    let cal = Calendar {
        id: Uuid::new_v4().to_string(),
        name: cal_name.clone(),
        color: source_color(source_type),
        source_type: source_type.to_string(),
        source_url: if url_or_path.starts_with("http") {
            Some(url_or_path)
        } else {
            None
        },
        visible: true,
        auto_sync: false,
        last_synced: Some(now_iso()),
        created_at: now_iso(),
    };

    // Parse events
    let events = parse_ics(&content, &cal.id, source);
    let imported_count = events.len();

    // Save calendar and events
    let mut calendars = read_calendars()?;
    calendars.push(cal.clone());
    write_calendars(&calendars)?;
    write_events(&cal.id, &events)?;

    Ok(ImportResult {
        calendar_id: cal.id,
        calendar_name: cal_name,
        imported_count,
        skipped_count: 0,
        errors: Vec::new(),
    })
}

/// Default color per source type.
fn source_color(source_type: &str) -> String {
    match source_type {
        "google" => "#4285f4".to_string(),
        "outlook" => "#0078d4".to_string(),
        "apple" => "#ff3b30".to_string(),
        "ics" => "#8b5cf6".to_string(),
        _ => "#22c55e".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands — Event CRUD
// ---------------------------------------------------------------------------

/// List events within a date range, optionally filtered by calendar IDs.
#[tauri::command]
pub async fn calendar_list_events(
    start_date: String,
    end_date: String,
    calendar_ids: Option<Vec<String>>,
) -> AppResult<Vec<CalendarEvent>> {
    let calendars = read_calendars()?;

    let target_cals: Vec<&Calendar> = if let Some(ref ids) = calendar_ids {
        calendars.iter().filter(|c| ids.contains(&c.id) && c.visible).collect()
    } else {
        calendars.iter().filter(|c| c.visible).collect()
    };

    let mut all_events = Vec::new();

    for cal in target_cals {
        let events = read_events(&cal.id)?;
        for evt in events {
            // Filter by date range (compare ISO strings lexicographically)
            let evt_start = evt.start.get(..10).unwrap_or("");
            let range_start = start_date.get(..10).unwrap_or("");
            let range_end = end_date.get(..10).unwrap_or("");

            if evt_start >= range_start && evt_start <= range_end {
                all_events.push(evt);
            }
        }
    }

    // Sort by start time
    all_events.sort_by(|a, b| a.start.cmp(&b.start));
    Ok(all_events)
}

/// Create a new event.
#[tauri::command]
pub async fn calendar_create_event(event: CreateEventInput) -> AppResult<CalendarEvent> {
    // Verify calendar exists
    let calendars = read_calendars()?;
    if !calendars.iter().any(|c| c.id == event.calendar_id) {
        return Err(ImpForgeError::validation(
            "CAL_NOT_FOUND",
            format!("Calendar {} not found", event.calendar_id),
        ));
    }

    let now = now_iso();
    let new_event = CalendarEvent {
        id: Uuid::new_v4().to_string(),
        title: event.title,
        description: event.description,
        start: event.start,
        end: event.end,
        all_day: event.all_day,
        location: event.location,
        color: event.color,
        calendar_id: event.calendar_id.clone(),
        recurrence: event.recurrence,
        reminder_minutes: event.reminder_minutes,
        attendees: event.attendees,
        source: EventSource::Local,
        created_at: now.clone(),
        updated_at: now,
    };

    let mut events = read_events(&event.calendar_id)?;
    events.push(new_event.clone());
    write_events(&event.calendar_id, &events)?;

    Ok(new_event)
}

/// Update an existing event.
#[tauri::command]
pub async fn calendar_update_event(id: String, updates: UpdateEventInput) -> AppResult<CalendarEvent> {
    let calendars = read_calendars()?;

    // Find which calendar holds this event
    for cal in &calendars {
        let mut events = read_events(&cal.id)?;
        if let Some(evt) = events.iter_mut().find(|e| e.id == id) {
            if let Some(title) = updates.title {
                evt.title = title;
            }
            if let Some(desc) = updates.description {
                evt.description = desc;
            }
            if let Some(start) = updates.start {
                evt.start = start;
            }
            if let Some(end) = updates.end {
                evt.end = end;
            }
            if let Some(all_day) = updates.all_day {
                evt.all_day = all_day;
            }
            if let Some(loc) = updates.location {
                evt.location = loc;
            }
            if let Some(color) = updates.color {
                evt.color = color;
            }
            if let Some(cal_id) = updates.calendar_id {
                evt.calendar_id = cal_id;
            }
            if let Some(rec) = updates.recurrence {
                evt.recurrence = rec;
            }
            if let Some(rem) = updates.reminder_minutes {
                evt.reminder_minutes = rem;
            }
            if let Some(att) = updates.attendees {
                evt.attendees = att;
            }
            evt.updated_at = now_iso();

            let updated = evt.clone();
            write_events(&cal.id, &events)?;
            return Ok(updated);
        }
    }

    Err(ImpForgeError::validation(
        "EVT_NOT_FOUND",
        format!("Event {id} not found in any calendar"),
    ))
}

/// Delete an event by ID.
#[tauri::command]
pub async fn calendar_delete_event(id: String) -> AppResult<()> {
    let calendars = read_calendars()?;

    for cal in &calendars {
        let mut events = read_events(&cal.id)?;
        let before = events.len();
        events.retain(|e| e.id != id);

        if events.len() < before {
            write_events(&cal.id, &events)?;
            return Ok(());
        }
    }

    Err(ImpForgeError::validation(
        "EVT_NOT_FOUND",
        format!("Event {id} not found in any calendar"),
    ))
}

/// Get a day's agenda with all events.
#[tauri::command]
pub async fn calendar_get_day(date: String) -> AppResult<DayAgenda> {
    let events = calendar_list_events(date.clone(), date.clone(), None).await?;

    Ok(DayAgenda {
        date,
        events,
        ai_summary: None,
    })
}

// ---------------------------------------------------------------------------
// Tauri Commands — AI Features
// ---------------------------------------------------------------------------

/// Suggest optimal meeting times based on existing events.
#[tauri::command]
pub async fn calendar_ai_suggest_time(
    duration_minutes: u32,
    _participants: Vec<String>,
    preferred_hours: Option<String>,
) -> AppResult<Vec<TimeSlot>> {
    // Get today and next 7 days of events
    let today = Utc::now().date_naive();
    let start_date = today.format("%Y-%m-%d").to_string();
    let end_date = (today + Duration::days(7)).format("%Y-%m-%d").to_string();

    let events = calendar_list_events(start_date.clone(), end_date.clone(), None).await?;

    let (pref_start, pref_end) = parse_preferred_hours(preferred_hours.as_deref());

    let mut slots = Vec::new();

    // Check each day for free slots
    for day_offset in 0..7 {
        let check_date = today + Duration::days(day_offset);
        let date_str = check_date.format("%Y-%m-%d").to_string();

        // Get events for this day
        let day_events: Vec<&CalendarEvent> = events
            .iter()
            .filter(|e| e.start.starts_with(&date_str) && !e.all_day)
            .collect();

        // Find free slots within preferred hours
        let mut busy_ranges: Vec<(u32, u32)> = Vec::new();
        for evt in &day_events {
            let start_min = time_to_minutes(&evt.start);
            let end_min = time_to_minutes(&evt.end);
            if let (Some(s), Some(e)) = (start_min, end_min) {
                busy_ranges.push((s, e));
            }
        }
        busy_ranges.sort_by_key(|r| r.0);

        // Scan for gaps
        let mut cursor = pref_start * 60;
        let day_end = pref_end * 60;

        for (busy_start, busy_end) in &busy_ranges {
            if cursor + duration_minutes <= *busy_start && cursor + duration_minutes <= day_end {
                let slot_start = minutes_to_datetime(&date_str, cursor);
                let slot_end = minutes_to_datetime(&date_str, cursor + duration_minutes);
                let score = score_time_slot(cursor, day_offset, pref_start, pref_end);

                slots.push(TimeSlot {
                    start: slot_start,
                    end: slot_end,
                    score,
                    reason: format_slot_reason(cursor, day_offset),
                });

                if slots.len() >= 5 {
                    break;
                }
            }
            cursor = cursor.max(*busy_end);
        }

        // Check remaining time after last event
        if cursor + duration_minutes <= day_end && slots.len() < 5 {
            let slot_start = minutes_to_datetime(&date_str, cursor);
            let slot_end = minutes_to_datetime(&date_str, cursor + duration_minutes);
            let score = score_time_slot(cursor, day_offset, pref_start, pref_end);

            slots.push(TimeSlot {
                start: slot_start,
                end: slot_end,
                score,
                reason: format_slot_reason(cursor, day_offset),
            });
        }

        if slots.len() >= 5 {
            break;
        }
    }

    // Sort by score (highest first)
    slots.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    slots.truncate(5);

    Ok(slots)
}

/// Generate an AI daily briefing for a given date.
#[tauri::command]
pub async fn calendar_ai_daily_briefing(date: String) -> AppResult<String> {
    let agenda = calendar_get_day(date.clone()).await?;

    if agenda.events.is_empty() {
        return Ok(format!("You have no events scheduled for {date}. Your day is free!"));
    }

    let events_summary: Vec<String> = agenda.events.iter().map(|e| {
        let time = if e.all_day {
            "All day".to_string()
        } else {
            let start = e.start.get(11..16).unwrap_or("??:??");
            let end = e.end.get(11..16).unwrap_or("??:??");
            format!("{start} - {end}")
        };
        let loc = e.location.as_deref().unwrap_or("");
        let loc_str = if loc.is_empty() { String::new() } else { format!(" at {loc}") };
        format!("- {time}: {}{loc_str}", e.title)
    }).collect();

    let system_prompt = "You are a professional AI assistant inside ImpForge, an AI Workstation. \
        Generate a concise, friendly daily briefing for the user based on their calendar events. \
        Highlight the most important events, suggest preparation tips, and note any scheduling concerns \
        (back-to-back meetings, long gaps, etc.). Keep it under 200 words.";

    let user_message = format!(
        "Date: {date}\nTotal events: {}\n\nSchedule:\n{}",
        agenda.events.len(),
        events_summary.join("\n")
    );

    ollama_chat(system_prompt, &user_message, None).await
}

/// Generate a meeting agenda for a specific event.
#[tauri::command]
pub async fn calendar_ai_generate_agenda(event_id: String) -> AppResult<String> {
    let calendars = read_calendars()?;

    // Find the event
    let mut target_event: Option<CalendarEvent> = None;
    for cal in &calendars {
        let events = read_events(&cal.id)?;
        if let Some(evt) = events.into_iter().find(|e| e.id == event_id) {
            target_event = Some(evt);
            break;
        }
    }

    let event = target_event.ok_or_else(|| {
        ImpForgeError::validation("EVT_NOT_FOUND", format!("Event {event_id} not found"))
    })?;

    let system_prompt = "You are a professional AI assistant inside ImpForge. \
        Generate a structured meeting agenda based on the event details provided. \
        Include: purpose, talking points, time allocation, action items section, \
        and notes section. Format in clean Markdown. Keep it professional and actionable.";

    let attendees_str = if event.attendees.is_empty() {
        "Not specified".to_string()
    } else {
        event.attendees.join(", ")
    };

    let user_message = format!(
        "Event: {}\nDate/Time: {} to {}\nLocation: {}\nDescription: {}\nAttendees: {}\n\n\
         Generate a professional meeting agenda for this event.",
        event.title,
        event.start,
        event.end,
        event.location.as_deref().unwrap_or("Not specified"),
        event.description.as_deref().unwrap_or("No description provided"),
        attendees_str,
    );

    ollama_chat(system_prompt, &user_message, None).await
}

// ---------------------------------------------------------------------------
// Tauri Commands — Sync
// ---------------------------------------------------------------------------

/// Re-sync a calendar from its ICS source URL.
#[tauri::command]
pub async fn calendar_sync_ics(calendar_id: String) -> AppResult<ImportResult> {
    let mut calendars = read_calendars()?;

    let cal = calendars
        .iter_mut()
        .find(|c| c.id == calendar_id)
        .ok_or_else(|| {
            ImpForgeError::validation(
                "CAL_NOT_FOUND",
                format!("Calendar {calendar_id} not found"),
            )
        })?;

    let url = cal.source_url.clone().ok_or_else(|| {
        ImpForgeError::validation(
            "NO_SOURCE_URL",
            "This calendar has no source URL configured for sync",
        )
        .with_suggestion("Only calendars imported from a URL can be synced.")
    })?;

    // Fetch ICS content
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| {
            ImpForgeError::internal("HTTP_CLIENT", format!("Failed to build HTTP client: {e}"))
        })?;

    let resp = client.get(&url).send().await.map_err(|e| {
        ImpForgeError::service(
            "ICS_SYNC_FAILED",
            format!("Failed to fetch ICS for sync: {e}"),
        )
    })?;

    if !resp.status().is_success() {
        return Err(ImpForgeError::service(
            "ICS_SYNC_HTTP",
            format!("ICS sync URL returned HTTP {}", resp.status()),
        ));
    }

    let content = resp.text().await.map_err(|e| {
        ImpForgeError::service("ICS_SYNC_READ", format!("Failed to read sync response: {e}"))
    })?;

    let source = detect_ics_source(&content);
    let events = parse_ics(&content, &calendar_id, source);
    let imported_count = events.len();

    // Replace all events for this calendar
    write_events(&calendar_id, &events)?;

    // Update last_synced and extract name before releasing the mutable borrow
    cal.last_synced = Some(now_iso());
    let cal_name = cal.name.clone();
    write_calendars(&calendars)?;

    Ok(ImportResult {
        calendar_id,
        calendar_name: cal_name,
        imported_count,
        skipped_count: 0,
        errors: Vec::new(),
    })
}

// ---------------------------------------------------------------------------
// Time utility helpers
// ---------------------------------------------------------------------------

/// Parse preferred hours string like "9-17" into (start_hour, end_hour).
fn parse_preferred_hours(pref: Option<&str>) -> (u32, u32) {
    if let Some(s) = pref {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() == 2 {
            let start = parts[0].trim().parse::<u32>().unwrap_or(9);
            let end = parts[1].trim().parse::<u32>().unwrap_or(17);
            return (start.min(23), end.min(24));
        }
    }
    (9, 17) // default: 9 AM to 5 PM
}

/// Extract minutes-since-midnight from an ISO datetime string.
fn time_to_minutes(iso: &str) -> Option<u32> {
    // Format: "2026-03-15T14:30:00..."
    let time_part = iso.get(11..16)?;
    let parts: Vec<&str> = time_part.split(':').collect();
    if parts.len() >= 2 {
        let h = parts[0].parse::<u32>().ok()?;
        let m = parts[1].parse::<u32>().ok()?;
        Some(h * 60 + m)
    } else {
        None
    }
}

/// Convert minutes-since-midnight to an ISO datetime string.
fn minutes_to_datetime(date: &str, minutes: u32) -> String {
    let h = (minutes / 60) % 24;
    let m = minutes % 60;
    format!("{date}T{h:02}:{m:02}:00")
}

/// Score a time slot (higher = better). Prefers morning, sooner days.
fn score_time_slot(minutes: u32, day_offset: i64, pref_start: u32, pref_end: u32) -> f64 {
    let hours = minutes as f64 / 60.0;
    let pref_mid = (pref_start as f64 + pref_end as f64) / 2.0;

    // Prefer slots closer to mid-morning
    let ideal_hour = pref_mid - 1.0;
    let time_score = 1.0 - (hours - ideal_hour).abs() / 12.0;

    // Prefer sooner days
    let day_score = 1.0 - (day_offset as f64 * 0.1);

    (time_score * 0.6 + day_score * 0.4).clamp(0.0, 1.0)
}

/// Generate a human-readable reason for a suggested time slot.
fn format_slot_reason(minutes: u32, day_offset: i64) -> String {
    let h = minutes / 60;
    let period = if h < 12 { "morning" } else if h < 17 { "afternoon" } else { "evening" };
    let day_label = match day_offset {
        0 => "today".to_string(),
        1 => "tomorrow".to_string(),
        _ => format!("in {day_offset} days"),
    };
    format!("Free {period} slot {day_label}")
}

// ---------------------------------------------------------------------------
// Tauri Commands — Recurring Events
// ---------------------------------------------------------------------------

/// Create a recurring event from a structured recurrence rule.
///
/// The `recurrence_rule` is converted to an RFC 5545 RRULE string and stored
/// on the event. The base event represents the first occurrence; the frontend
/// calls `calendar_expand_recurrence` to materialize instances within a range.
#[tauri::command]
pub async fn calendar_create_recurring(
    event: CreateEventInput,
    recurrence_rule: RecurrenceRule,
) -> AppResult<CalendarEvent> {
    if recurrence_rule.interval == 0 {
        return Err(ImpForgeError::validation(
            "INVALID_INTERVAL",
            "Recurrence interval must be at least 1",
        ));
    }

    let calendars = read_calendars()?;
    if !calendars.iter().any(|c| c.id == event.calendar_id) {
        return Err(ImpForgeError::validation(
            "CAL_NOT_FOUND",
            format!("Calendar {} not found", event.calendar_id),
        ));
    }

    let now = now_iso();
    let rrule_str = recurrence_rule.to_rrule_string();

    let new_event = CalendarEvent {
        id: Uuid::new_v4().to_string(),
        title: event.title,
        description: event.description,
        start: event.start,
        end: event.end,
        all_day: event.all_day,
        location: event.location,
        color: event.color,
        calendar_id: event.calendar_id.clone(),
        recurrence: Some(rrule_str),
        reminder_minutes: event.reminder_minutes,
        attendees: event.attendees,
        source: EventSource::Local,
        created_at: now.clone(),
        updated_at: now,
    };

    let mut events = read_events(&event.calendar_id)?;
    events.push(new_event.clone());
    write_events(&event.calendar_id, &events)?;

    log::info!(
        "ForgeCalendar: created recurring event '{}' ({})",
        new_event.title,
        new_event.id
    );

    Ok(new_event)
}

/// Expand recurring events within a date range.
///
/// Returns virtual occurrences of recurring events that fall within the given
/// range. Non-recurring events in the range are also included. Each occurrence
/// carries the parent event ID and adjusted start/end times.
#[tauri::command]
pub async fn calendar_expand_recurrence(
    start_date: String,
    end_date: String,
    calendar_ids: Option<Vec<String>>,
) -> AppResult<Vec<CalendarEvent>> {
    let calendars = read_calendars()?;

    let target_cals: Vec<&Calendar> = if let Some(ref ids) = calendar_ids {
        calendars.iter().filter(|c| ids.contains(&c.id) && c.visible).collect()
    } else {
        calendars.iter().filter(|c| c.visible).collect()
    };

    let mut all_events = Vec::new();

    for cal in target_cals {
        let events = read_events(&cal.id)?;
        for evt in &events {
            if let Some(ref rrule_str) = evt.recurrence {
                // Expand recurring event
                if let Some(rule) = RecurrenceRule::from_rrule_string(rrule_str) {
                    let occurrences =
                        rule.expand_occurrences(&evt.start, &evt.end, &start_date, &end_date);
                    for (occ_start, occ_end) in occurrences {
                        let mut occ = evt.clone();
                        occ.start = occ_start;
                        occ.end = occ_end;
                        all_events.push(occ);
                    }
                } else {
                    // Fallback: treat as non-recurring
                    let evt_start = evt.start.get(..10).unwrap_or("");
                    let range_start = start_date.get(..10).unwrap_or("");
                    let range_end = end_date.get(..10).unwrap_or("");
                    if evt_start >= range_start && evt_start <= range_end {
                        all_events.push(evt.clone());
                    }
                }
            } else {
                // Non-recurring: simple date range filter
                let evt_start = evt.start.get(..10).unwrap_or("");
                let range_start = start_date.get(..10).unwrap_or("");
                let range_end = end_date.get(..10).unwrap_or("");
                if evt_start >= range_start && evt_start <= range_end {
                    all_events.push(evt.clone());
                }
            }
        }
    }

    all_events.sort_by(|a, b| a.start.cmp(&b.start));
    Ok(all_events)
}

/// Parse a recurrence rule string and return the structured representation.
#[tauri::command]
pub async fn calendar_parse_rrule(rrule: String) -> AppResult<RecurrenceRule> {
    RecurrenceRule::from_rrule_string(&rrule).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_RRULE",
            format!("Cannot parse RRULE: {rrule}"),
        )
        .with_suggestion("Expected format: FREQ=DAILY;INTERVAL=1;BYDAY=MO,WE,FR")
    })
}

// ---------------------------------------------------------------------------
// Tauri Commands — Reminders
// ---------------------------------------------------------------------------

/// Get upcoming event reminders within the next N minutes.
///
/// Scans all visible calendars for events with `reminder_minutes` set, then
/// returns those whose reminder trigger falls within `[now, now + minutes_ahead]`.
#[tauri::command]
pub async fn calendar_upcoming_reminders(minutes_ahead: u32) -> AppResult<Vec<EventReminder>> {
    let calendars = read_calendars()?;
    let now = Utc::now();
    let now_str = now.format("%Y-%m-%dT%H:%M:%S").to_string();
    let horizon = now + Duration::minutes(minutes_ahead as i64);
    let horizon_str = horizon.format("%Y-%m-%dT%H:%M:%S").to_string();

    let mut reminders = Vec::new();

    for cal in &calendars {
        if !cal.visible {
            continue;
        }
        let events = read_events(&cal.id)?;
        for evt in &events {
            let Some(rem_mins) = evt.reminder_minutes else {
                continue;
            };

            // Parse event start time
            let evt_start_trimmed = evt.start.trim_end_matches('Z');
            let Some(evt_dt) = NaiveDateTime::parse_from_str(
                evt_start_trimmed.get(..19).unwrap_or(evt_start_trimmed),
                "%Y-%m-%dT%H:%M:%S",
            )
            .ok()
            else {
                continue;
            };

            // Reminder fires at: event_start - reminder_minutes
            let reminder_at = evt_dt - Duration::minutes(rem_mins as i64);
            let reminder_str = reminder_at.format("%Y-%m-%dT%H:%M:%S").to_string();

            // Check if the reminder falls within [now, now+horizon]
            if reminder_str >= now_str && reminder_str <= horizon_str {
                let minutes_until = (evt_dt - now.naive_utc()).num_minutes();
                reminders.push(EventReminder {
                    event_id: evt.id.clone(),
                    event_title: evt.title.clone(),
                    event_start: evt.start.clone(),
                    reminder_at: reminder_str,
                    minutes_until,
                    calendar_name: cal.name.clone(),
                    location: evt.location.clone(),
                });
            }
        }
    }

    reminders.sort_by(|a, b| a.reminder_at.cmp(&b.reminder_at));
    Ok(reminders)
}

// ---------------------------------------------------------------------------
// Tauri Commands — ICS Export
// ---------------------------------------------------------------------------

/// Export a calendar as an ICS/iCal string (RFC 5545 VCALENDAR).
///
/// The exported string can be saved to a `.ics` file or shared with other
/// calendar applications.
#[tauri::command]
pub async fn calendar_export_ics(calendar_id: String) -> AppResult<String> {
    let calendars = read_calendars()?;
    let cal = calendars
        .iter()
        .find(|c| c.id == calendar_id)
        .ok_or_else(|| {
            ImpForgeError::validation(
                "CAL_NOT_FOUND",
                format!("Calendar {calendar_id} not found"),
            )
        })?;

    let events = read_events(&calendar_id)?;
    let mut ics = String::with_capacity(4096);

    ics.push_str("BEGIN:VCALENDAR\r\n");
    ics.push_str("VERSION:2.0\r\n");
    ics.push_str("PRODID:-//ImpForge//ForgeCalendar//EN\r\n");
    ics.push_str("CALSCALE:GREGORIAN\r\n");
    ics.push_str("METHOD:PUBLISH\r\n");
    ics.push_str(&format!("X-WR-CALNAME:{}\r\n", escape_ics_text(&cal.name)));

    for evt in &events {
        ics.push_str("BEGIN:VEVENT\r\n");
        ics.push_str(&format!("UID:{}\r\n", evt.id));

        // DTSTART / DTEND
        if evt.all_day {
            let start_compact = evt.start.replace('-', "");
            let end_compact = evt.end.replace('-', "");
            ics.push_str(&format!("DTSTART;VALUE=DATE:{}\r\n", start_compact.get(..8).unwrap_or(&start_compact)));
            ics.push_str(&format!("DTEND;VALUE=DATE:{}\r\n", end_compact.get(..8).unwrap_or(&end_compact)));
        } else {
            ics.push_str(&format!("DTSTART:{}\r\n", to_ics_datetime(&evt.start)));
            ics.push_str(&format!("DTEND:{}\r\n", to_ics_datetime(&evt.end)));
        }

        ics.push_str(&format!("SUMMARY:{}\r\n", escape_ics_text(&evt.title)));

        if let Some(ref desc) = evt.description {
            ics.push_str(&format!("DESCRIPTION:{}\r\n", escape_ics_text(desc)));
        }
        if let Some(ref loc) = evt.location {
            ics.push_str(&format!("LOCATION:{}\r\n", escape_ics_text(loc)));
        }
        if let Some(ref rrule) = evt.recurrence {
            ics.push_str(&format!("RRULE:{rrule}\r\n"));
        }

        for attendee in &evt.attendees {
            if attendee.contains('@') {
                ics.push_str(&format!("ATTENDEE:mailto:{attendee}\r\n"));
            } else {
                ics.push_str(&format!("ATTENDEE;CN={attendee}:mailto:\r\n"));
            }
        }

        if let Some(rem_mins) = evt.reminder_minutes {
            ics.push_str("BEGIN:VALARM\r\n");
            ics.push_str("ACTION:DISPLAY\r\n");
            ics.push_str("DESCRIPTION:Reminder\r\n");
            ics.push_str(&format!("TRIGGER:-PT{rem_mins}M\r\n"));
            ics.push_str("END:VALARM\r\n");
        }

        ics.push_str("END:VEVENT\r\n");
    }

    ics.push_str("END:VCALENDAR\r\n");

    log::info!(
        "ForgeCalendar: exported {} events from '{}' as ICS",
        events.len(),
        cal.name
    );

    Ok(ics)
}

/// Escape text for ICS output (reverse of `unescape_ics_text`).
fn escape_ics_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(',', "\\,")
        .replace(';', "\\;")
}

/// Convert an ISO datetime string to ICS compact format.
/// `2026-03-15T14:00:00Z` -> `20260315T140000Z`
fn to_ics_datetime(iso: &str) -> String {
    let mut result = String::with_capacity(16);
    for ch in iso.chars() {
        if ch != '-' && ch != ':' {
            result.push(ch);
        }
    }
    // Ensure it ends with Z if no timezone suffix
    if !result.ends_with('Z') && !result.contains('+') {
        result.push('Z');
    }
    result
}

// ---------------------------------------------------------------------------
// Tauri Commands — Timezone & Week Number
// ---------------------------------------------------------------------------

/// Get ISO week number information for a given date.
///
/// Returns the ISO 8601 week number, year, day-of-week name, and day-of-year.
#[tauri::command]
pub async fn calendar_week_info(date: String) -> AppResult<WeekInfo> {
    let parsed = NaiveDate::parse_from_str(
        date.get(..10).unwrap_or(&date),
        "%Y-%m-%d",
    )
    .map_err(|e| {
        ImpForgeError::validation(
            "INVALID_DATE",
            format!("Cannot parse date '{date}': {e}"),
        )
        .with_suggestion("Use ISO 8601 format: YYYY-MM-DD")
    })?;

    let iso_week: IsoWeek = parsed.iso_week();
    let day_name = match parsed.weekday() {
        Weekday::Mon => "Monday",
        Weekday::Tue => "Tuesday",
        Weekday::Wed => "Wednesday",
        Weekday::Thu => "Thursday",
        Weekday::Fri => "Friday",
        Weekday::Sat => "Saturday",
        Weekday::Sun => "Sunday",
    };

    Ok(WeekInfo {
        date: parsed.format("%Y-%m-%d").to_string(),
        iso_week: iso_week.week(),
        iso_year: iso_week.year(),
        day_of_week: day_name.to_string(),
        day_of_year: parsed.ordinal(),
    })
}

/// Get ISO week numbers for an entire month, grouped by week.
///
/// Returns a list of `(week_number, [date_strings])` for the given year/month.
#[tauri::command]
pub async fn calendar_month_weeks(year: i32, month: u32) -> AppResult<Vec<(u32, Vec<String>)>> {
    if !(1..=12).contains(&month) {
        return Err(ImpForgeError::validation(
            "INVALID_MONTH",
            format!("Month must be 1-12, got {month}"),
        ));
    }

    let first = NaiveDate::from_ymd_opt(year, month, 1).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_DATE",
            format!("Cannot construct date for {year}-{month:02}"),
        )
    })?;

    let num_days = days_in_month(year, month);
    let mut weeks: Vec<(u32, Vec<String>)> = Vec::new();

    for day in 1..=num_days {
        let Some(d) = NaiveDate::from_ymd_opt(year, month, day) else {
            continue;
        };
        let wk = d.iso_week().week();
        let date_str = d.format("%Y-%m-%d").to_string();

        if let Some(last) = weeks.last_mut() {
            if last.0 == wk {
                last.1.push(date_str);
                continue;
            }
        }
        weeks.push((wk, vec![date_str]));
    }

    let _ = first; // used above via from_ymd_opt
    Ok(weeks)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ics_datetime_utc() {
        assert_eq!(parse_ics_datetime("20260315T140000Z"), "2026-03-15T14:00:00Z");
    }

    #[test]
    fn test_parse_ics_datetime_local() {
        assert_eq!(parse_ics_datetime("20260315T093000"), "2026-03-15T09:30:00");
    }

    #[test]
    fn test_parse_ics_datetime_all_day() {
        assert_eq!(parse_ics_datetime("20260315"), "2026-03-15");
    }

    #[test]
    fn test_parse_ics_duration_minutes() {
        assert_eq!(parse_ics_duration_minutes("-PT15M"), Some(15));
        assert_eq!(parse_ics_duration_minutes("-PT1H"), Some(60));
        assert_eq!(parse_ics_duration_minutes("-PT1H30M"), Some(90));
        assert_eq!(parse_ics_duration_minutes("-P1D"), Some(1440));
        assert_eq!(parse_ics_duration_minutes("-P1W"), Some(10080));
    }

    #[test]
    fn test_unescape_ics_text() {
        assert_eq!(unescape_ics_text("Hello\\nWorld"), "Hello\nWorld");
        assert_eq!(unescape_ics_text("A\\,B\\;C"), "A,B;C");
        assert_eq!(unescape_ics_text("Back\\\\slash"), "Back\\slash");
    }

    #[test]
    fn test_strip_ics_prop() {
        assert_eq!(strip_ics_prop("SUMMARY:Team Meeting", "SUMMARY"), Some("Team Meeting"));
        assert_eq!(strip_ics_prop("SUMMARY;LANGUAGE=en:Team Meeting", "SUMMARY"), Some("Team Meeting"));
        assert_eq!(strip_ics_prop("DESCRIPTION:Hello", "SUMMARY"), None);
    }

    #[test]
    fn test_extract_ics_value() {
        assert_eq!(extract_ics_value("DTSTART;TZID=America/New_York:20260315T140000"), "20260315T140000");
        assert_eq!(extract_ics_value("DTSTART:20260315T140000Z"), "20260315T140000Z");
        assert_eq!(extract_ics_value("DTSTART;VALUE=DATE:20260315"), "20260315");
    }

    #[test]
    fn test_parse_ics_full_event() {
        let ics = r#"BEGIN:VCALENDAR
VERSION:2.0
PRODID:-//Google Inc//Google Calendar 70.9054//EN
BEGIN:VEVENT
DTSTART:20260315T140000Z
DTEND:20260315T150000Z
SUMMARY:Team Standup
DESCRIPTION:Daily standup meeting\nAll team members
LOCATION:Conference Room B
ATTENDEE;CN=John Doe:mailto:john@example.com
ATTENDEE;CN=Jane Smith:mailto:jane@example.com
RRULE:FREQ=DAILY;BYDAY=MO,TU,WE,TH,FR
BEGIN:VALARM
TRIGGER:-PT15M
ACTION:DISPLAY
END:VALARM
END:VEVENT
END:VCALENDAR"#;

        let events = parse_ics(ics, "test-cal-id", EventSource::GoogleImport);
        assert_eq!(events.len(), 1);

        let evt = &events[0];
        assert_eq!(evt.title, "Team Standup");
        assert_eq!(evt.start, "2026-03-15T14:00:00Z");
        assert_eq!(evt.end, "2026-03-15T15:00:00Z");
        assert!(!evt.all_day);
        assert_eq!(evt.location.as_deref(), Some("Conference Room B"));
        assert_eq!(evt.description.as_deref(), Some("Daily standup meeting\nAll team members"));
        assert_eq!(evt.attendees.len(), 2);
        assert_eq!(evt.attendees[0], "john@example.com");
        assert_eq!(evt.reminder_minutes, Some(15));
        assert!(evt.recurrence.as_deref().unwrap_or("").contains("FREQ=DAILY"));
        assert_eq!(evt.calendar_id, "test-cal-id");
    }

    #[test]
    fn test_parse_ics_all_day_event() {
        let ics = r#"BEGIN:VCALENDAR
BEGIN:VEVENT
DTSTART;VALUE=DATE:20260325
DTEND;VALUE=DATE:20260326
SUMMARY:Company Holiday
END:VEVENT
END:VCALENDAR"#;

        let events = parse_ics(ics, "cal1", EventSource::Local);
        assert_eq!(events.len(), 1);
        assert!(events[0].all_day);
        assert_eq!(events[0].start, "2026-03-25");
        assert_eq!(events[0].end, "2026-03-26");
    }

    #[test]
    fn test_parse_ics_multiline_unfold() {
        let ics = "BEGIN:VCALENDAR\r\nBEGIN:VEVENT\r\nDTSTART:20260315T100000Z\r\nDTEND:20260315T110000Z\r\nSUMMARY:Long title that is\r\n  continued on the next line\r\nEND:VEVENT\r\nEND:VCALENDAR";

        let events = parse_ics(ics, "cal1", EventSource::Local);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].title, "Long title that is continued on the next line");
    }

    #[test]
    fn test_detect_ics_source() {
        let google = "PRODID:-//Google Inc//Google Calendar";
        let outlook = "PRODID:-//Microsoft Corporation";
        let apple = "PRODID:-//Apple Inc.";
        let other = "PRODID:-//Somebody//Calendar";

        assert_eq!(detect_ics_source(google), EventSource::GoogleImport);
        assert_eq!(detect_ics_source(outlook), EventSource::OutlookImport);
        assert_eq!(detect_ics_source(apple), EventSource::AppleImport);
        assert!(matches!(detect_ics_source(other), EventSource::IcsImport { .. }));
    }

    #[test]
    fn test_parse_preferred_hours() {
        assert_eq!(parse_preferred_hours(Some("9-17")), (9, 17));
        assert_eq!(parse_preferred_hours(Some("8 - 18")), (8, 18));
        assert_eq!(parse_preferred_hours(None), (9, 17));
        assert_eq!(parse_preferred_hours(Some("invalid")), (9, 17));
    }

    #[test]
    fn test_time_to_minutes() {
        assert_eq!(time_to_minutes("2026-03-15T14:30:00Z"), Some(870));
        assert_eq!(time_to_minutes("2026-03-15T00:00:00"), Some(0));
        assert_eq!(time_to_minutes("2026-03-15T23:59:00"), Some(1439));
        assert_eq!(time_to_minutes("2026-03-15"), None); // all-day
    }

    #[test]
    fn test_minutes_to_datetime() {
        assert_eq!(minutes_to_datetime("2026-03-15", 870), "2026-03-15T14:30:00");
        assert_eq!(minutes_to_datetime("2026-03-15", 0), "2026-03-15T00:00:00");
    }

    #[test]
    fn test_score_time_slot() {
        let morning = score_time_slot(600, 0, 9, 17);  // 10:00 today
        let evening = score_time_slot(1080, 0, 9, 17);  // 18:00 today
        let later_day = score_time_slot(600, 5, 9, 17); // 10:00 in 5 days

        // Morning today should score higher than evening or later days
        assert!(morning > evening);
        assert!(morning > later_day);
    }

    #[test]
    fn test_source_color() {
        assert_eq!(source_color("google"), "#4285f4");
        assert_eq!(source_color("outlook"), "#0078d4");
        assert_eq!(source_color("apple"), "#ff3b30");
        assert_eq!(source_color("ics"), "#8b5cf6");
    }

    #[test]
    fn test_extract_ics_calname() {
        let content = "BEGIN:VCALENDAR\nX-WR-CALNAME:Work Calendar\nBEGIN:VEVENT";
        assert_eq!(extract_ics_calname(content), Some("Work Calendar".to_string()));

        let no_name = "BEGIN:VCALENDAR\nBEGIN:VEVENT";
        assert_eq!(extract_ics_calname(no_name), None);
    }

    #[test]
    fn test_extract_ics_param() {
        assert_eq!(
            extract_ics_param("ATTENDEE;CN=John Doe:mailto:john@example.com", "CN"),
            Some("John Doe")
        );
        assert_eq!(
            extract_ics_param("ATTENDEE;CN=\"Jane Smith\";ROLE=REQ:mailto:jane@ex.com", "CN"),
            Some("Jane Smith")
        );
        assert_eq!(
            extract_ics_param("SUMMARY:Test", "CN"),
            None
        );
    }

    // --- Enterprise additions: Recurrence ---

    #[test]
    fn test_recurrence_rule_to_rrule_string() {
        let rule = RecurrenceRule {
            frequency: Frequency::Weekly,
            interval: 2,
            until: None,
            count: Some(10),
            by_day: vec!["MO".into(), "WE".into(), "FR".into()],
        };
        let rrule = rule.to_rrule_string();
        assert!(rrule.contains("FREQ=WEEKLY"));
        assert!(rrule.contains("INTERVAL=2"));
        assert!(rrule.contains("BYDAY=MO,WE,FR"));
        assert!(rrule.contains("COUNT=10"));
    }

    #[test]
    fn test_recurrence_rule_from_rrule_string() {
        let rrule = "FREQ=DAILY;INTERVAL=3;COUNT=5";
        let rule = RecurrenceRule::from_rrule_string(rrule).expect("should parse");
        assert_eq!(rule.frequency, Frequency::Daily);
        assert_eq!(rule.interval, 3);
        assert_eq!(rule.count, Some(5));
        assert!(rule.by_day.is_empty());
    }

    #[test]
    fn test_recurrence_rule_roundtrip() {
        let original = RecurrenceRule {
            frequency: Frequency::Monthly,
            interval: 1,
            until: Some("2027-01-01".into()),
            count: None,
            by_day: Vec::new(),
        };
        let rrule = original.to_rrule_string();
        let parsed = RecurrenceRule::from_rrule_string(&rrule).expect("roundtrip");
        assert_eq!(parsed.frequency, Frequency::Monthly);
        assert_eq!(parsed.interval, 1);
    }

    #[test]
    fn test_expand_daily_occurrences() {
        let rule = RecurrenceRule {
            frequency: Frequency::Daily,
            interval: 1,
            until: None,
            count: Some(5),
            by_day: Vec::new(),
        };
        let occs = rule.expand_occurrences(
            "2026-03-15T10:00:00",
            "2026-03-15T11:00:00",
            "2026-03-15",
            "2026-03-25",
        );
        assert_eq!(occs.len(), 5);
        assert!(occs[0].0.starts_with("2026-03-15"));
        assert!(occs[4].0.starts_with("2026-03-19"));
    }

    #[test]
    fn test_expand_weekly_byday() {
        let rule = RecurrenceRule {
            frequency: Frequency::Weekly,
            interval: 1,
            until: None,
            count: Some(6),
            by_day: vec!["MO".into(), "WE".into(), "FR".into()],
        };
        let occs = rule.expand_occurrences(
            "2026-03-16T09:00:00", // Monday
            "2026-03-16T10:00:00",
            "2026-03-16",
            "2026-04-30",
        );
        assert_eq!(occs.len(), 6);
    }

    #[test]
    fn test_days_in_month() {
        assert_eq!(days_in_month(2026, 1), 31);
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29); // leap year
        assert_eq!(days_in_month(2026, 4), 30);
        assert_eq!(days_in_month(2026, 12), 31);
    }

    // --- Enterprise additions: ICS Export ---

    #[test]
    fn test_escape_ics_text() {
        assert_eq!(escape_ics_text("Hello\nWorld"), "Hello\\nWorld");
        assert_eq!(escape_ics_text("A,B;C"), "A\\,B\\;C");
        assert_eq!(escape_ics_text("Back\\slash"), "Back\\\\slash");
    }

    #[test]
    fn test_to_ics_datetime() {
        assert_eq!(to_ics_datetime("2026-03-15T14:00:00Z"), "20260315T140000Z");
        assert_eq!(to_ics_datetime("2026-03-15T14:00:00"), "20260315T140000Z");
    }

    // --- Enterprise additions: Week Info ---

    #[tokio::test]
    async fn test_calendar_week_info() {
        let info = calendar_week_info("2026-01-05".into()).await.expect("should work");
        assert_eq!(info.iso_week, 2);
        assert_eq!(info.day_of_week, "Monday");
    }

    #[tokio::test]
    async fn test_calendar_month_weeks() {
        let weeks = calendar_month_weeks(2026, 3).await.expect("should work");
        assert!(!weeks.is_empty());
        // March 2026 should have 5 weeks
        assert!(weeks.len() >= 4);
        // Sum of all dates should be 31
        let total_days: usize = weeks.iter().map(|(_, dates)| dates.len()).sum();
        assert_eq!(total_days, 31);
    }
}
