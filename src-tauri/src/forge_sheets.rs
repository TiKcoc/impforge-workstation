// SPDX-License-Identifier: Apache-2.0
//! ForgeSheets -- KI-native Spreadsheet Engine (Excel replacement)
//!
//! Zero-Entry spreadsheet: users write natural language, AI generates formulas.
//! Canvas-based rendering intent for millions of cells (frontend handles rendering).
//! Import/Export: .xlsx, .csv, .tsv, .json, .ods via `calamine` (read) and
//! `rust_xlsxwriter` (write).
//!
//! Clean Room implementation — NO Microsoft code, NO Office libraries.
//!
//! Research:
//! - arXiv:2510.15585 — TDD + LLM for spreadsheet formula generation
//! - arXiv:2403.03636 — SheetAgent: autonomous spreadsheet reasoning
//! - arXiv:2406.14991 — SpreadsheetBench: real-world manipulation
//! - arXiv:2402.14853 — NL2Formula: natural language to formulas
//! - calamine crate (MIT) — read .xlsx/.xls/.ods/.csv
//! - rust_xlsxwriter crate (MIT) — write .xlsx
//!
//! This module is part of ImpForge Phase 3 (Office tools).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use calamine::{open_workbook_auto, Data, Reader};
use chrono::{Local, Utc};
use rust_xlsxwriter::{Format, Workbook};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppResult, ImpForgeError};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Subdirectory under `~/.impforge/` for spreadsheet storage.
const SHEETS_DIR: &str = "spreadsheets";

/// Ollama timeout for AI formula generation (generous for complex descriptions).
const AI_FORMULA_TIMEOUT_SECS: u64 = 90;

/// Maximum columns supported (A..ZZ = 702).
const MAX_COLS: u32 = 702;

/// Maximum rows before we warn (rendering concern, not a hard limit).
/// Reserved for future CSV import row-cap enforcement.
#[allow(dead_code)]
const MAX_ROWS: u32 = 1_048_576;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single cell value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum CellValue {
    Empty,
    Text(String),
    Number(f64),
    Bool(bool),
    Error(String),
    /// In-cell sparkline chart. `data` holds numeric values, `chart_type` is
    /// "line" or "bar". The frontend renders these as tiny inline SVGs.
    Sparkline {
        data: Vec<f64>,
        chart_type: String,
    },
}

impl Default for CellValue {
    fn default() -> Self {
        CellValue::Empty
    }
}

/// Text alignment within a cell.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
}

/// Visual formatting for a cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellFormat {
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    /// Hex color string (e.g. "#ff0000").
    pub text_color: Option<String>,
    /// Hex color string for background.
    pub bg_color: Option<String>,
    /// Number format string (e.g. "#,##0.00", "0%", "yyyy-mm-dd").
    pub number_format: Option<String>,
    #[serde(default)]
    pub align: TextAlign,
}

impl Default for CellFormat {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            text_color: None,
            bg_color: None,
            number_format: None,
            align: TextAlign::Left,
        }
    }
}

/// A single cell in a sheet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cell {
    pub value: CellValue,
    /// Formula string, e.g. "=SUM(A1:A10)".
    pub formula: Option<String>,
    #[serde(default)]
    pub format: CellFormat,
    /// User note / comment.
    pub note: Option<String>,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            value: CellValue::Empty,
            formula: None,
            format: CellFormat::default(),
            note: None,
        }
    }
}

/// A worksheet within a spreadsheet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sheet {
    pub name: String,
    /// Map of cell references ("A1", "B2") to cell data.
    pub cells: HashMap<String, Cell>,
    /// Custom column widths (column index -> width in chars).
    pub col_widths: HashMap<u32, f32>,
    /// Custom row heights (row index -> height in pixels).
    pub row_heights: HashMap<u32, f32>,
    /// Agentic cells that can autonomously fetch/compute data.
    #[serde(default)]
    pub agentic_cells: HashMap<String, AgenticCell>,
    /// Conditional formatting rules.
    #[serde(default)]
    pub conditional_formats: Vec<ConditionalFormatRule>,
    /// Data validation rules.
    #[serde(default)]
    pub data_validations: Vec<DataValidationRule>,
    /// Chart configurations stored on this sheet.
    #[serde(default)]
    pub charts: Vec<ChartConfig>,
}

impl Sheet {
    fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cells: HashMap::new(),
            col_widths: HashMap::new(),
            row_heights: HashMap::new(),
            agentic_cells: HashMap::new(),
            conditional_formats: Vec::new(),
            data_validations: Vec::new(),
            charts: Vec::new(),
        }
    }
}

/// A complete spreadsheet document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Spreadsheet {
    pub id: String,
    pub name: String,
    pub sheets: Vec<Sheet>,
    pub created_at: String,
    pub updated_at: String,
}

/// Lightweight metadata for spreadsheet listings (no cell data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpreadsheetMeta {
    pub id: String,
    pub name: String,
    pub sheet_count: usize,
    pub cell_count: usize,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// Agentic Cells — cells that autonomously fetch/compute data
// Research: arXiv:2403.03636 (SheetAgent)
// ---------------------------------------------------------------------------

/// What kind of autonomous data source an agentic cell uses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgenticType {
    /// Fetch data from a URL.
    WebFetch { url: String },
    /// Call an HTTP API endpoint.
    ApiCall {
        endpoint: String,
        method: String,
        headers: Vec<(String, String)>,
    },
    /// Generate content via local AI (Ollama).
    AiGenerate { prompt: String },
    /// Watch a local file for changes.
    FileWatch { path: String },
    /// Auto-recalculating formula.
    Formula { expression: String },
}

/// A cell that can autonomously fetch or compute its value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgenticCell {
    pub cell_ref: String,
    pub agent_type: AgenticType,
    pub config: serde_json::Value,
    /// Refresh interval in seconds. 0 = manual only.
    pub refresh_interval: Option<u32>,
    pub last_fetched: Option<String>,
}

// ---------------------------------------------------------------------------
// Chart types
// ---------------------------------------------------------------------------

/// Supported chart types for visualization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    Bar,
    Line,
    Pie,
    Scatter,
    Area,
}

/// Full chart configuration for rendering in the frontend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartConfig {
    pub chart_type: ChartType,
    pub title: String,
    pub data_range: String,
    pub labels_range: Option<String>,
    pub colors: Vec<String>,
    /// Extracted data points (series of numeric values) for frontend rendering.
    pub series: Vec<ChartSeries>,
    /// Category labels (X-axis or pie labels).
    pub categories: Vec<String>,
}

/// A single data series for chart rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSeries {
    pub name: String,
    pub values: Vec<f64>,
}

// ---------------------------------------------------------------------------
// Conditional Formatting & Data Validation
// ---------------------------------------------------------------------------

/// A conditional formatting rule applied to a range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalFormatRule {
    pub id: String,
    pub range: String,
    pub condition: ConditionalType,
    /// Background color to apply when the condition is met (hex).
    pub bg_color: Option<String>,
    /// Text color to apply when the condition is met (hex).
    pub text_color: Option<String>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
}

/// The type of conditional test.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConditionalType {
    GreaterThan { value: f64 },
    LessThan { value: f64 },
    EqualTo { value: f64 },
    Between { min: f64, max: f64 },
    TextContains { text: String },
    IsEmpty,
    IsNotEmpty,
}

/// Data validation constraint for a cell or range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataValidationRule {
    pub id: String,
    pub range: String,
    pub validation: ValidationType,
    pub error_message: Option<String>,
}

/// What kind of validation to enforce.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValidationType {
    /// Only allow values from a predefined list.
    DropdownList { options: Vec<String> },
    /// Only allow numbers within a range.
    NumberRange { min: Option<f64>, max: Option<f64> },
    /// Only allow text with a max length.
    TextLength { max: usize },
    /// Only allow dates (ISO 8601).
    DateOnly,
}

// ---------------------------------------------------------------------------
// Pivot Table
// ---------------------------------------------------------------------------

/// Aggregation function for pivot tables.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PivotAggregation {
    Sum,
    Average,
    Count,
    Min,
    Max,
}

/// Configuration for generating a pivot table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PivotConfig {
    pub data_range: String,
    pub row_field: usize,
    pub col_field: usize,
    pub value_field: usize,
    pub aggregation: PivotAggregation,
}

/// Result row from a pivot table computation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PivotResult {
    pub headers: Vec<String>,
    pub rows: Vec<PivotRow>,
}

/// A single row in the pivot result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PivotRow {
    pub label: String,
    pub values: Vec<Option<f64>>,
}

/// Result from Auto-EDA analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisResult {
    pub summary: String,
    pub trends: Vec<String>,
    pub outliers: Vec<OutlierInfo>,
    pub correlations: Vec<CorrelationInfo>,
    pub suggested_charts: Vec<ChartSuggestion>,
    pub stats: RangeStats,
}

/// Info about a detected outlier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutlierInfo {
    pub cell_ref: String,
    pub value: f64,
    pub reason: String,
}

/// Info about a detected correlation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrelationInfo {
    pub columns: (String, String),
    pub coefficient: f64,
    pub description: String,
}

/// A suggested chart type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSuggestion {
    pub chart_type: String,
    pub reason: String,
    pub data_range: String,
}

/// Basic statistics for a numeric range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeStats {
    pub count: usize,
    pub sum: f64,
    pub average: f64,
    pub min: f64,
    pub max: f64,
    pub std_dev: f64,
}

// ---------------------------------------------------------------------------
// Storage helpers
// ---------------------------------------------------------------------------

fn sheets_dir() -> Result<PathBuf, ImpForgeError> {
    let base = dirs::home_dir().ok_or_else(|| {
        ImpForgeError::filesystem("HOME_DIR", "Cannot determine home directory")
    })?;
    let dir = base.join(".impforge").join(SHEETS_DIR);
    if !dir.exists() {
        std::fs::create_dir_all(&dir).map_err(|e| {
            ImpForgeError::filesystem(
                "DIR_CREATE_FAILED",
                format!("Failed to create spreadsheets directory: {e}"),
            )
        })?;
    }
    Ok(dir)
}

fn spreadsheet_path(dir: &Path, id: &str) -> PathBuf {
    dir.join(format!("{id}.json"))
}

fn load_spreadsheet(dir: &Path, id: &str) -> Result<Spreadsheet, ImpForgeError> {
    let path = spreadsheet_path(dir, id);
    if !path.exists() {
        return Err(
            ImpForgeError::filesystem(
                "SHEET_NOT_FOUND",
                format!("Spreadsheet '{id}' not found"),
            )
            .with_suggestion("The spreadsheet may have been deleted."),
        );
    }
    let data = std::fs::read_to_string(&path).map_err(|e| {
        ImpForgeError::filesystem("READ_FAILED", format!("Cannot read spreadsheet: {e}"))
    })?;
    serde_json::from_str::<Spreadsheet>(&data).map_err(|e| {
        ImpForgeError::internal(
            "PARSE_FAILED",
            format!("Corrupt spreadsheet data: {e}"),
        )
    })
}

fn save_spreadsheet(dir: &Path, sheet: &Spreadsheet) -> Result<(), ImpForgeError> {
    let path = spreadsheet_path(dir, &sheet.id);
    let json = serde_json::to_string_pretty(sheet).map_err(|e| {
        ImpForgeError::internal("SERIALIZE_FAILED", format!("Cannot serialize spreadsheet: {e}"))
    })?;
    std::fs::write(&path, json).map_err(|e| {
        ImpForgeError::filesystem("WRITE_FAILED", format!("Cannot save spreadsheet: {e}"))
    })
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

// ---------------------------------------------------------------------------
// Cell reference helpers
// ---------------------------------------------------------------------------

/// Convert column number (0-based) to letter(s): 0->A, 1->B, 25->Z, 26->AA.
fn col_to_letter(mut col: u32) -> String {
    let mut result = String::new();
    loop {
        result.insert(0, (b'A' + (col % 26) as u8) as char);
        if col < 26 {
            break;
        }
        col = col / 26 - 1;
    }
    result
}

/// Parse column letter(s) to 0-based number: A->0, B->1, Z->25, AA->26.
fn letter_to_col(letters: &str) -> Option<u32> {
    if letters.is_empty() {
        return None;
    }
    let mut result: u32 = 0;
    for ch in letters.chars() {
        if !ch.is_ascii_uppercase() {
            return None;
        }
        result = result
            .checked_mul(26)?
            .checked_add((ch as u32) - ('A' as u32) + 1)?;
    }
    Some(result.checked_sub(1)?)
}

/// Parse a cell reference like "A1" into (col_0based, row_0based).
fn parse_cell_ref(cell_ref: &str) -> Option<(u32, u32)> {
    let col_end = cell_ref
        .chars()
        .position(|c| c.is_ascii_digit())?;
    if col_end == 0 {
        return None;
    }
    let col_str = &cell_ref[..col_end];
    let row_str = &cell_ref[col_end..];
    let col = letter_to_col(col_str)?;
    let row: u32 = row_str.parse().ok()?;
    if row == 0 {
        return None; // Rows are 1-based in spreadsheet notation
    }
    Some((col, row - 1))
}

/// Build a cell reference string from 0-based col, row.
fn make_cell_ref(col: u32, row: u32) -> String {
    format!("{}{}", col_to_letter(col), row + 1)
}

/// Parse a range like "A1:B10" into (start_col, start_row, end_col, end_row).
/// All values are 0-based.
fn parse_range(range: &str) -> Option<(u32, u32, u32, u32)> {
    let parts: Vec<&str> = range.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let (sc, sr) = parse_cell_ref(parts[0])?;
    let (ec, er) = parse_cell_ref(parts[1])?;
    Some((sc.min(ec), sr.min(er), sc.max(ec), sr.max(er)))
}

// ---------------------------------------------------------------------------
// Formula Engine — basic evaluation
// ---------------------------------------------------------------------------

/// Resolve a single cell's numeric value from the cells map.
fn resolve_numeric(cells: &HashMap<String, Cell>, cell_ref: &str) -> Option<f64> {
    match cells.get(cell_ref) {
        Some(cell) => match &cell.value {
            CellValue::Number(n) => Some(*n),
            CellValue::Text(s) => s.trim().parse::<f64>().ok(),
            CellValue::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            CellValue::Sparkline { .. } | CellValue::Empty | CellValue::Error(_) => None,
        },
        None => None,
    }
}

/// Resolve a single cell's string value from the cells map.
fn resolve_string(cells: &HashMap<String, Cell>, cell_ref: &str) -> String {
    match cells.get(cell_ref) {
        Some(cell) => match &cell.value {
            CellValue::Text(s) => s.clone(),
            CellValue::Number(n) => {
                if *n == n.floor() && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    format!("{n}")
                }
            }
            CellValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.to_string(),
            CellValue::Error(e) => format!("#ERR:{e}"),
            CellValue::Sparkline { chart_type, data } => {
                format!("SPARKLINE({chart_type},{} pts)", data.len())
            }
            CellValue::Empty => String::new(),
        },
        None => String::new(),
    }
}

/// Collect all numeric values from a range.
fn collect_range_numbers(cells: &HashMap<String, Cell>, range: &str) -> Vec<f64> {
    let Some((sc, sr, ec, er)) = parse_range(range) else {
        return Vec::new();
    };
    let mut values = Vec::new();
    for row in sr..=er {
        for col in sc..=ec {
            let ref_str = make_cell_ref(col, row);
            if let Some(n) = resolve_numeric(cells, &ref_str) {
                values.push(n);
            }
        }
    }
    values
}

/// Parse a formula argument that is either a range ("A1:A10") or a single cell
/// reference ("A1") or a literal number.
fn parse_arg_numbers(cells: &HashMap<String, Cell>, arg: &str) -> Vec<f64> {
    let arg = arg.trim();
    if arg.contains(':') {
        collect_range_numbers(cells, arg)
    } else if let Some(_) = parse_cell_ref(arg) {
        resolve_numeric(cells, arg).into_iter().collect()
    } else if let Ok(n) = arg.parse::<f64>() {
        vec![n]
    } else {
        Vec::new()
    }
}

/// Split function arguments respecting parentheses nesting.
fn split_args(args_str: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;
    let mut in_string = false;

    for ch in args_str.chars() {
        match ch {
            '"' => {
                in_string = !in_string;
                current.push(ch);
            }
            '(' if !in_string => {
                depth += 1;
                current.push(ch);
            }
            ')' if !in_string => {
                depth -= 1;
                current.push(ch);
            }
            ',' if !in_string && depth == 0 => {
                result.push(current.trim().to_string());
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }
    let remaining = current.trim().to_string();
    if !remaining.is_empty() {
        result.push(remaining);
    }
    result
}

/// Evaluate a single expression (either a literal, cell ref, or sub-formula).
fn eval_expr(cells: &HashMap<String, Cell>, expr: &str) -> CellValue {
    let expr = expr.trim();

    // Empty
    if expr.is_empty() {
        return CellValue::Empty;
    }

    // String literal "..."
    if expr.starts_with('"') && expr.ends_with('"') && expr.len() >= 2 {
        return CellValue::Text(expr[1..expr.len() - 1].to_string());
    }

    // Boolean literals
    if expr.eq_ignore_ascii_case("true") {
        return CellValue::Bool(true);
    }
    if expr.eq_ignore_ascii_case("false") {
        return CellValue::Bool(false);
    }

    // Numeric literal
    if let Ok(n) = expr.parse::<f64>() {
        return CellValue::Number(n);
    }

    // Cell reference
    if let Some(_) = parse_cell_ref(expr) {
        return match resolve_numeric(cells, expr) {
            Some(n) => CellValue::Number(n),
            None => {
                let s = resolve_string(cells, expr);
                if s.is_empty() {
                    CellValue::Empty
                } else {
                    CellValue::Text(s)
                }
            }
        };
    }

    // Sub-formula
    if expr.starts_with('=') {
        return evaluate_formula_inner(cells, &expr[1..]);
    }

    // If it looks like a function call, try evaluating
    if let Some(paren_pos) = expr.find('(') {
        if expr.ends_with(')') {
            return evaluate_formula_inner(cells, expr);
        }
        let _ = paren_pos; // suppress unused warning
    }

    CellValue::Text(expr.to_string())
}

/// Evaluate a numeric expression that may contain +, -, *, / operators
/// with cell references and literals. Simple left-to-right, no precedence
/// beyond * and / before + and - (two-pass).
fn eval_arithmetic(cells: &HashMap<String, Cell>, expr: &str) -> Option<f64> {
    let expr = expr.trim();

    // Try direct number
    if let Ok(n) = expr.parse::<f64>() {
        return Some(n);
    }

    // Try cell reference
    if let Some(_) = parse_cell_ref(expr) {
        return resolve_numeric(cells, expr);
    }

    // Tokenize into numbers and operators
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for ch in expr.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            '+' | '-' if depth == 0 && !current.is_empty() => {
                tokens.push(current.trim().to_string());
                tokens.push(ch.to_string());
                current.clear();
            }
            '*' | '/' if depth == 0 && !current.is_empty() => {
                tokens.push(current.trim().to_string());
                tokens.push(ch.to_string());
                current.clear();
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.trim().is_empty() {
        tokens.push(current.trim().to_string());
    }

    if tokens.is_empty() {
        return None;
    }

    // Resolve all value tokens to numbers
    let mut values: Vec<f64> = Vec::new();
    let mut ops: Vec<String> = Vec::new();

    for token in &tokens {
        match token.as_str() {
            "+" | "-" | "*" | "/" => ops.push(token.clone()),
            _ => {
                let val = if let Ok(n) = token.parse::<f64>() {
                    n
                } else if let Some(_) = parse_cell_ref(token) {
                    resolve_numeric(cells, token)?
                } else if token.contains('(') {
                    // Sub-expression / function
                    match evaluate_formula_inner(cells, token) {
                        CellValue::Number(n) => n,
                        _ => return None,
                    }
                } else {
                    return None;
                };
                values.push(val);
            }
        }
    }

    if values.is_empty() {
        return None;
    }
    if values.len() != ops.len() + 1 {
        return None;
    }

    // Pass 1: * and /
    let mut v2 = vec![values[0]];
    let mut o2: Vec<String> = Vec::new();
    for (i, op) in ops.iter().enumerate() {
        match op.as_str() {
            "*" => {
                let last = v2.last_mut()?;
                *last *= values[i + 1];
            }
            "/" => {
                if values[i + 1] == 0.0 {
                    return None; // #DIV/0!
                }
                let last = v2.last_mut()?;
                *last /= values[i + 1];
            }
            _ => {
                v2.push(values[i + 1]);
                o2.push(op.clone());
            }
        }
    }

    // Pass 2: + and -
    let mut result = v2[0];
    for (i, op) in o2.iter().enumerate() {
        match op.as_str() {
            "+" => result += v2[i + 1],
            "-" => result -= v2[i + 1],
            _ => {}
        }
    }

    Some(result)
}

/// Inner formula evaluator. The input should NOT include the leading '='.
fn evaluate_formula_inner(cells: &HashMap<String, Cell>, formula: &str) -> CellValue {
    let formula = formula.trim();

    // Try as function call: FUNC(args...)
    if let Some(paren_pos) = formula.find('(') {
        if formula.ends_with(')') {
            let func_name = formula[..paren_pos].trim().to_uppercase();
            let args_str = &formula[paren_pos + 1..formula.len() - 1];

            match func_name.as_str() {
                "SUM" => {
                    let args = split_args(args_str);
                    let mut total = 0.0;
                    for arg in &args {
                        for n in parse_arg_numbers(cells, arg) {
                            total += n;
                        }
                    }
                    return CellValue::Number(total);
                }
                "AVERAGE" | "AVG" => {
                    let args = split_args(args_str);
                    let mut total = 0.0;
                    let mut count = 0usize;
                    for arg in &args {
                        for n in parse_arg_numbers(cells, arg) {
                            total += n;
                            count += 1;
                        }
                    }
                    return if count > 0 {
                        CellValue::Number(total / count as f64)
                    } else {
                        CellValue::Error("DIV/0".to_string())
                    };
                }
                "COUNT" => {
                    let args = split_args(args_str);
                    let mut count = 0usize;
                    for arg in &args {
                        count += parse_arg_numbers(cells, arg).len();
                    }
                    return CellValue::Number(count as f64);
                }
                "MIN" => {
                    let args = split_args(args_str);
                    let mut all_nums = Vec::new();
                    for arg in &args {
                        all_nums.extend(parse_arg_numbers(cells, arg));
                    }
                    return if let Some(min) = all_nums.iter().copied().reduce(f64::min) {
                        CellValue::Number(min)
                    } else {
                        CellValue::Number(0.0)
                    };
                }
                "MAX" => {
                    let args = split_args(args_str);
                    let mut all_nums = Vec::new();
                    for arg in &args {
                        all_nums.extend(parse_arg_numbers(cells, arg));
                    }
                    return if let Some(max) = all_nums.iter().copied().reduce(f64::max) {
                        CellValue::Number(max)
                    } else {
                        CellValue::Number(0.0)
                    };
                }
                "IF" => {
                    let args = split_args(args_str);
                    if args.len() < 2 {
                        return CellValue::Error("IF requires 2-3 arguments".to_string());
                    }
                    let condition = eval_condition(cells, &args[0]);
                    return if condition {
                        eval_expr(cells, &args[1])
                    } else if args.len() >= 3 {
                        eval_expr(cells, &args[2])
                    } else {
                        CellValue::Bool(false)
                    };
                }
                "CONCAT" | "CONCATENATE" => {
                    let args = split_args(args_str);
                    let mut result = String::new();
                    for arg in &args {
                        let arg = arg.trim();
                        if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
                            result.push_str(&arg[1..arg.len() - 1]);
                        } else if let Some(_) = parse_cell_ref(arg) {
                            result.push_str(&resolve_string(cells, arg));
                        } else {
                            result.push_str(arg);
                        }
                    }
                    return CellValue::Text(result);
                }
                "ABS" => {
                    let args = split_args(args_str);
                    if let Some(arg) = args.first() {
                        let nums = parse_arg_numbers(cells, arg);
                        if let Some(n) = nums.first() {
                            return CellValue::Number(n.abs());
                        }
                    }
                    return CellValue::Error("ABS requires 1 argument".to_string());
                }
                "ROUND" => {
                    let args = split_args(args_str);
                    if args.len() < 2 {
                        return CellValue::Error("ROUND requires 2 arguments".to_string());
                    }
                    let nums = parse_arg_numbers(cells, &args[0]);
                    let decimals = parse_arg_numbers(cells, &args[1]);
                    if let (Some(n), Some(d)) = (nums.first(), decimals.first()) {
                        let factor = 10f64.powi(*d as i32);
                        return CellValue::Number((n * factor).round() / factor);
                    }
                    return CellValue::Error("ROUND: invalid arguments".to_string());
                }
                "LEN" => {
                    let args = split_args(args_str);
                    if let Some(arg) = args.first() {
                        let arg = arg.trim();
                        let s = if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
                            arg[1..arg.len() - 1].to_string()
                        } else if let Some(_) = parse_cell_ref(arg) {
                            resolve_string(cells, arg)
                        } else {
                            arg.to_string()
                        };
                        return CellValue::Number(s.len() as f64);
                    }
                    return CellValue::Error("LEN requires 1 argument".to_string());
                }
                "UPPER" => {
                    let args = split_args(args_str);
                    if let Some(arg) = args.first() {
                        let arg = arg.trim();
                        let s = if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
                            arg[1..arg.len() - 1].to_string()
                        } else if let Some(_) = parse_cell_ref(arg) {
                            resolve_string(cells, arg)
                        } else {
                            arg.to_string()
                        };
                        return CellValue::Text(s.to_uppercase());
                    }
                    return CellValue::Error("UPPER requires 1 argument".to_string());
                }
                "LOWER" => {
                    let args = split_args(args_str);
                    if let Some(arg) = args.first() {
                        let arg = arg.trim();
                        let s = if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
                            arg[1..arg.len() - 1].to_string()
                        } else if let Some(_) = parse_cell_ref(arg) {
                            resolve_string(cells, arg)
                        } else {
                            arg.to_string()
                        };
                        return CellValue::Text(s.to_lowercase());
                    }
                    return CellValue::Error("LOWER requires 1 argument".to_string());
                }

                // ----- Extended Formula Functions (Enterprise) -----

                // VLOOKUP(value, range, col_index)
                "VLOOKUP" => {
                    let args = split_args(args_str);
                    if args.len() < 3 {
                        return CellValue::Error(
                            "VLOOKUP requires 3 arguments: value, range, col_index"
                                .to_string(),
                        );
                    }
                    let search_val = eval_expr(cells, &args[0]);
                    let range_str = args[1].trim();
                    let col_offset = match parse_arg_numbers(cells, &args[2])
                        .first()
                        .copied()
                    {
                        Some(n) if n >= 1.0 => n as u32,
                        _ => {
                            return CellValue::Error(
                                "VLOOKUP: col_index must be >= 1".to_string(),
                            )
                        }
                    };
                    let Some((sc, sr, ec, er)) = parse_range(range_str) else {
                        return CellValue::Error(
                            "VLOOKUP: invalid range".to_string(),
                        );
                    };
                    if col_offset as u32 > (ec - sc + 1) {
                        return CellValue::Error(
                            "VLOOKUP: col_index exceeds range width".to_string(),
                        );
                    }
                    // Search first column for match
                    for row in sr..=er {
                        let first_col_ref = make_cell_ref(sc, row);
                        let cell_val = eval_expr(cells, &first_col_ref);
                        if cell_values_equal(&search_val, &cell_val) {
                            let result_ref =
                                make_cell_ref(sc + col_offset - 1, row);
                            return eval_expr(cells, &result_ref);
                        }
                    }
                    return CellValue::Error("VLOOKUP: value not found".to_string());
                }

                // HLOOKUP(value, range, row_index)
                "HLOOKUP" => {
                    let args = split_args(args_str);
                    if args.len() < 3 {
                        return CellValue::Error(
                            "HLOOKUP requires 3 arguments: value, range, row_index"
                                .to_string(),
                        );
                    }
                    let search_val = eval_expr(cells, &args[0]);
                    let range_str = args[1].trim();
                    let row_offset = match parse_arg_numbers(cells, &args[2])
                        .first()
                        .copied()
                    {
                        Some(n) if n >= 1.0 => n as u32,
                        _ => {
                            return CellValue::Error(
                                "HLOOKUP: row_index must be >= 1".to_string(),
                            )
                        }
                    };
                    let Some((sc, sr, ec, er)) = parse_range(range_str) else {
                        return CellValue::Error(
                            "HLOOKUP: invalid range".to_string(),
                        );
                    };
                    if row_offset > (er - sr + 1) {
                        return CellValue::Error(
                            "HLOOKUP: row_index exceeds range height".to_string(),
                        );
                    }
                    // Search first row for match
                    for col in sc..=ec {
                        let first_row_ref = make_cell_ref(col, sr);
                        let cell_val = eval_expr(cells, &first_row_ref);
                        if cell_values_equal(&search_val, &cell_val) {
                            let result_ref =
                                make_cell_ref(col, sr + row_offset - 1);
                            return eval_expr(cells, &result_ref);
                        }
                    }
                    return CellValue::Error("HLOOKUP: value not found".to_string());
                }

                // INDEX(range, row, col)
                "INDEX" => {
                    let args = split_args(args_str);
                    if args.len() < 3 {
                        return CellValue::Error(
                            "INDEX requires 3 arguments: range, row, col"
                                .to_string(),
                        );
                    }
                    let range_str = args[0].trim();
                    let row_idx = parse_arg_numbers(cells, &args[1])
                        .first()
                        .copied()
                        .unwrap_or(0.0) as u32;
                    let col_idx = parse_arg_numbers(cells, &args[2])
                        .first()
                        .copied()
                        .unwrap_or(0.0) as u32;
                    let Some((sc, sr, _ec, _er)) = parse_range(range_str) else {
                        return CellValue::Error(
                            "INDEX: invalid range".to_string(),
                        );
                    };
                    if row_idx < 1 || col_idx < 1 {
                        return CellValue::Error(
                            "INDEX: row and col must be >= 1".to_string(),
                        );
                    }
                    let target_ref =
                        make_cell_ref(sc + col_idx - 1, sr + row_idx - 1);
                    return eval_expr(cells, &target_ref);
                }

                // MATCH(value, range) — returns 1-based position
                "MATCH" => {
                    let args = split_args(args_str);
                    if args.len() < 2 {
                        return CellValue::Error(
                            "MATCH requires 2 arguments: value, range"
                                .to_string(),
                        );
                    }
                    let search_val = eval_expr(cells, &args[0]);
                    let range_str = args[1].trim();
                    let Some((sc, sr, ec, er)) = parse_range(range_str) else {
                        return CellValue::Error(
                            "MATCH: invalid range".to_string(),
                        );
                    };
                    // Determine if range is a row or column
                    let is_single_row = sr == er;
                    let mut position = 1u32;
                    if is_single_row {
                        for col in sc..=ec {
                            let ref_str = make_cell_ref(col, sr);
                            let cell_val = eval_expr(cells, &ref_str);
                            if cell_values_equal(&search_val, &cell_val) {
                                return CellValue::Number(position as f64);
                            }
                            position += 1;
                        }
                    } else {
                        for row in sr..=er {
                            let ref_str = make_cell_ref(sc, row);
                            let cell_val = eval_expr(cells, &ref_str);
                            if cell_values_equal(&search_val, &cell_val) {
                                return CellValue::Number(position as f64);
                            }
                            position += 1;
                        }
                    }
                    return CellValue::Error("MATCH: value not found".to_string());
                }

                // SUMIF(range, criteria, sum_range)
                "SUMIF" => {
                    let args = split_args(args_str);
                    if args.len() < 2 {
                        return CellValue::Error(
                            "SUMIF requires 2-3 arguments: range, criteria[, sum_range]"
                                .to_string(),
                        );
                    }
                    let crit_range_str = args[0].trim();
                    let criteria = args[1].trim();
                    let sum_range_str = if args.len() >= 3 {
                        args[2].trim().to_string()
                    } else {
                        crit_range_str.to_string()
                    };
                    let Some((sc, sr, ec, er)) = parse_range(crit_range_str)
                    else {
                        return CellValue::Error(
                            "SUMIF: invalid criteria range".to_string(),
                        );
                    };
                    let sum_range_parsed = parse_range(&sum_range_str);
                    let (ssc, ssr, _, _) =
                        sum_range_parsed.unwrap_or((sc, sr, ec, er));
                    let mut total = 0.0;
                    for row in sr..=er {
                        for col in sc..=ec {
                            let ref_str = make_cell_ref(col, row);
                            if eval_criteria_match(cells, &ref_str, criteria) {
                                let sum_ref = make_cell_ref(
                                    ssc + (col - sc),
                                    ssr + (row - sr),
                                );
                                if let Some(n) =
                                    resolve_numeric(cells, &sum_ref)
                                {
                                    total += n;
                                }
                            }
                        }
                    }
                    return CellValue::Number(total);
                }

                // COUNTIF(range, criteria)
                "COUNTIF" => {
                    let args = split_args(args_str);
                    if args.len() < 2 {
                        return CellValue::Error(
                            "COUNTIF requires 2 arguments: range, criteria"
                                .to_string(),
                        );
                    }
                    let range_str = args[0].trim();
                    let criteria = args[1].trim();
                    let Some((sc, sr, ec, er)) = parse_range(range_str) else {
                        return CellValue::Error(
                            "COUNTIF: invalid range".to_string(),
                        );
                    };
                    let mut count = 0u32;
                    for row in sr..=er {
                        for col in sc..=ec {
                            let ref_str = make_cell_ref(col, row);
                            if eval_criteria_match(cells, &ref_str, criteria) {
                                count += 1;
                            }
                        }
                    }
                    return CellValue::Number(count as f64);
                }

                // TRIM(text)
                "TRIM" => {
                    let args = split_args(args_str);
                    if let Some(arg) = args.first() {
                        let s = resolve_string_arg(cells, arg.trim());
                        return CellValue::Text(s.split_whitespace().collect::<Vec<&str>>().join(" "));
                    }
                    return CellValue::Error("TRIM requires 1 argument".to_string());
                }

                // LEFT(text, n)
                "LEFT" => {
                    let args = split_args(args_str);
                    if args.is_empty() {
                        return CellValue::Error(
                            "LEFT requires 1-2 arguments".to_string(),
                        );
                    }
                    let s = resolve_string_arg(cells, args[0].trim());
                    let n = if args.len() >= 2 {
                        parse_arg_numbers(cells, &args[1])
                            .first()
                            .copied()
                            .unwrap_or(1.0) as usize
                    } else {
                        1
                    };
                    let result: String = s.chars().take(n).collect();
                    return CellValue::Text(result);
                }

                // RIGHT(text, n)
                "RIGHT" => {
                    let args = split_args(args_str);
                    if args.is_empty() {
                        return CellValue::Error(
                            "RIGHT requires 1-2 arguments".to_string(),
                        );
                    }
                    let s = resolve_string_arg(cells, args[0].trim());
                    let n = if args.len() >= 2 {
                        parse_arg_numbers(cells, &args[1])
                            .first()
                            .copied()
                            .unwrap_or(1.0) as usize
                    } else {
                        1
                    };
                    let chars: Vec<char> = s.chars().collect();
                    let start = chars.len().saturating_sub(n);
                    let result: String = chars[start..].iter().collect();
                    return CellValue::Text(result);
                }

                // MID(text, start, n)
                "MID" => {
                    let args = split_args(args_str);
                    if args.len() < 3 {
                        return CellValue::Error(
                            "MID requires 3 arguments: text, start, length"
                                .to_string(),
                        );
                    }
                    let s = resolve_string_arg(cells, args[0].trim());
                    let start = parse_arg_numbers(cells, &args[1])
                        .first()
                        .copied()
                        .unwrap_or(1.0) as usize;
                    let n = parse_arg_numbers(cells, &args[2])
                        .first()
                        .copied()
                        .unwrap_or(0.0) as usize;
                    if start < 1 {
                        return CellValue::Error(
                            "MID: start must be >= 1".to_string(),
                        );
                    }
                    let chars: Vec<char> = s.chars().collect();
                    let begin = (start - 1).min(chars.len());
                    let end = (begin + n).min(chars.len());
                    let result: String = chars[begin..end].iter().collect();
                    return CellValue::Text(result);
                }

                // NOW() — returns current date+time
                "NOW" => {
                    let now = Local::now();
                    return CellValue::Text(
                        now.format("%Y-%m-%d %H:%M:%S").to_string(),
                    );
                }

                // TODAY() — returns current date
                "TODAY" => {
                    let today = Local::now();
                    return CellValue::Text(
                        today.format("%Y-%m-%d").to_string(),
                    );
                }

                // AND(a, b, ...)
                "AND" => {
                    let args = split_args(args_str);
                    if args.is_empty() {
                        return CellValue::Error(
                            "AND requires at least 1 argument".to_string(),
                        );
                    }
                    for arg in &args {
                        if !eval_condition(cells, arg.trim()) {
                            return CellValue::Bool(false);
                        }
                    }
                    return CellValue::Bool(true);
                }

                // OR(a, b, ...)
                "OR" => {
                    let args = split_args(args_str);
                    if args.is_empty() {
                        return CellValue::Error(
                            "OR requires at least 1 argument".to_string(),
                        );
                    }
                    for arg in &args {
                        if eval_condition(cells, arg.trim()) {
                            return CellValue::Bool(true);
                        }
                    }
                    return CellValue::Bool(false);
                }

                // NOT(a)
                "NOT" => {
                    let args = split_args(args_str);
                    if let Some(arg) = args.first() {
                        return CellValue::Bool(
                            !eval_condition(cells, arg.trim()),
                        );
                    }
                    return CellValue::Error(
                        "NOT requires 1 argument".to_string(),
                    );
                }

                // IFERROR(value, fallback)
                "IFERROR" => {
                    let args = split_args(args_str);
                    if args.len() < 2 {
                        return CellValue::Error(
                            "IFERROR requires 2 arguments".to_string(),
                        );
                    }
                    let result = eval_expr(cells, &args[0]);
                    if matches!(result, CellValue::Error(_)) {
                        return eval_expr(cells, &args[1]);
                    }
                    return result;
                }

                // SPARKLINE(range, "line"|"bar") — in-cell mini chart
                "SPARKLINE" => {
                    let args = split_args(args_str);
                    if args.is_empty() {
                        return CellValue::Error(
                            "SPARKLINE requires 1-2 arguments: range[, type]".to_string(),
                        );
                    }
                    let data = parse_arg_numbers(cells, args[0].trim());
                    if data.is_empty() {
                        return CellValue::Error(
                            "SPARKLINE: no numeric data in range".to_string(),
                        );
                    }
                    let chart_type = if args.len() >= 2 {
                        let t = args[1].trim().trim_matches('"').to_lowercase();
                        match t.as_str() {
                            "bar" => "bar".to_string(),
                            _ => "line".to_string(),
                        }
                    } else {
                        "line".to_string()
                    };
                    return CellValue::Sparkline { data, chart_type };
                }

                _ => {
                    return CellValue::Error(format!("Unknown function: {func_name}"));
                }
            }
        }
    }

    // Try arithmetic expression
    if let Some(n) = eval_arithmetic(cells, formula) {
        return CellValue::Number(n);
    }

    // Fallback: treat as text
    CellValue::Text(formula.to_string())
}

/// Evaluate a condition string like "A1>10", "B2=5", "C3<>0".
fn eval_condition(cells: &HashMap<String, Cell>, cond: &str) -> bool {
    let cond = cond.trim();

    // Operators in order of length (longest first to avoid partial matches)
    let ops = ["<>", ">=", "<=", "!=", "=", ">", "<"];

    for op in &ops {
        if let Some(pos) = cond.find(op) {
            let left_str = cond[..pos].trim();
            let right_str = cond[pos + op.len()..].trim();

            let left_val = if let Some(_) = parse_cell_ref(left_str) {
                resolve_numeric(cells, left_str)
            } else {
                left_str.parse::<f64>().ok()
            };

            let right_val = if let Some(_) = parse_cell_ref(right_str) {
                resolve_numeric(cells, right_str)
            } else {
                right_str.parse::<f64>().ok()
            };

            if let (Some(l), Some(r)) = (left_val, right_val) {
                return match *op {
                    ">" => l > r,
                    "<" => l < r,
                    ">=" => l >= r,
                    "<=" => l <= r,
                    "=" => (l - r).abs() < f64::EPSILON,
                    "<>" | "!=" => (l - r).abs() >= f64::EPSILON,
                    _ => false,
                };
            }

            // String comparison fallback
            let left_s = if let Some(_) = parse_cell_ref(left_str) {
                resolve_string(cells, left_str)
            } else {
                left_str.trim_matches('"').to_string()
            };
            let right_s = if let Some(_) = parse_cell_ref(right_str) {
                resolve_string(cells, right_str)
            } else {
                right_str.trim_matches('"').to_string()
            };

            return match *op {
                "=" => left_s == right_s,
                "<>" | "!=" => left_s != right_s,
                _ => false,
            };
        }
    }

    // If the condition is just a cell ref or a literal, treat as truthy check
    if let Some(_) = parse_cell_ref(cond) {
        return resolve_numeric(cells, cond).map_or(false, |n| n != 0.0);
    }
    cond.parse::<f64>().map_or(!cond.is_empty(), |n| n != 0.0)
}

/// Compare two CellValues for equality (used by VLOOKUP, MATCH, etc.).
fn cell_values_equal(a: &CellValue, b: &CellValue) -> bool {
    match (a, b) {
        (CellValue::Number(x), CellValue::Number(y)) => (x - y).abs() < f64::EPSILON,
        (CellValue::Text(x), CellValue::Text(y)) => x.eq_ignore_ascii_case(y),
        (CellValue::Bool(x), CellValue::Bool(y)) => x == y,
        (CellValue::Number(n), CellValue::Text(s))
        | (CellValue::Text(s), CellValue::Number(n)) => {
            s.trim().parse::<f64>().map_or(false, |p| (p - n).abs() < f64::EPSILON)
        }
        _ => false,
    }
}

/// Resolve a formula argument to a string, handling literals and cell refs.
fn resolve_string_arg(cells: &HashMap<String, Cell>, arg: &str) -> String {
    let arg = arg.trim();
    if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
        arg[1..arg.len() - 1].to_string()
    } else if parse_cell_ref(arg).is_some() {
        resolve_string(cells, arg)
    } else {
        arg.to_string()
    }
}

/// Evaluate a SUMIF/COUNTIF criteria against a cell. Supports operators like
/// ">10", "<5", "=text", "<>0" and plain value matching.
fn eval_criteria_match(cells: &HashMap<String, Cell>, cell_ref: &str, criteria: &str) -> bool {
    let criteria = criteria.trim().trim_matches('"');

    // Operator-based criteria: ">10", ">=5", "<>0", etc.
    let ops: &[&str] = &["<>", ">=", "<=", "!=", ">", "<", "="];
    for op in ops {
        if let Some(rest) = criteria.strip_prefix(op) {
            let rest = rest.trim();
            // Try numeric comparison
            if let (Some(cell_num), Ok(crit_num)) =
                (resolve_numeric(cells, cell_ref), rest.parse::<f64>())
            {
                return match *op {
                    ">" => cell_num > crit_num,
                    "<" => cell_num < crit_num,
                    ">=" => cell_num >= crit_num,
                    "<=" => cell_num <= crit_num,
                    "=" => (cell_num - crit_num).abs() < f64::EPSILON,
                    "<>" | "!=" => (cell_num - crit_num).abs() >= f64::EPSILON,
                    _ => false,
                };
            }
            // String comparison
            let cell_str = resolve_string(cells, cell_ref);
            return match *op {
                "=" => cell_str.eq_ignore_ascii_case(rest),
                "<>" | "!=" => !cell_str.eq_ignore_ascii_case(rest),
                _ => false,
            };
        }
    }

    // Plain value match
    if let (Some(cell_num), Ok(crit_num)) =
        (resolve_numeric(cells, cell_ref), criteria.parse::<f64>())
    {
        return (cell_num - crit_num).abs() < f64::EPSILON;
    }
    let cell_str = resolve_string(cells, cell_ref);
    cell_str.eq_ignore_ascii_case(criteria)
}

/// Public formula evaluation entry point. Formula starts with '='.
fn evaluate_formula(cells: &HashMap<String, Cell>, formula: &str) -> CellValue {
    let formula = formula.trim();
    if !formula.starts_with('=') {
        return CellValue::Error("Formula must start with '='".to_string());
    }
    evaluate_formula_inner(cells, &formula[1..])
}

// ---------------------------------------------------------------------------
// Import helpers (calamine)
// ---------------------------------------------------------------------------

fn import_from_path(file_path: &Path) -> Result<Spreadsheet, ImpForgeError> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // CSV / TSV — handle specially
    if ext == "csv" || ext == "tsv" {
        return import_csv(file_path, if ext == "tsv" { b'\t' } else { b',' });
    }

    // JSON — our own format
    if ext == "json" {
        let data = std::fs::read_to_string(file_path).map_err(|e| {
            ImpForgeError::filesystem("IMPORT_READ", format!("Cannot read file: {e}"))
        })?;
        let mut ss: Spreadsheet = serde_json::from_str(&data).map_err(|e| {
            ImpForgeError::validation(
                "IMPORT_PARSE",
                format!("Invalid ForgeSheets JSON format: {e}"),
            )
        })?;
        ss.id = Uuid::new_v4().to_string();
        ss.updated_at = now_iso();
        return Ok(ss);
    }

    // .xlsx, .xls, .ods, .xlsb — use calamine
    let mut workbook = open_workbook_auto(file_path).map_err(|e| {
        ImpForgeError::validation(
            "IMPORT_OPEN",
            format!("Cannot open file as spreadsheet: {e}"),
        )
        .with_suggestion("Supported formats: .xlsx, .xls, .ods, .xlsb, .csv, .tsv, .json")
    })?;

    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    let name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported")
        .to_string();

    let mut sheets = Vec::new();

    for sn in &sheet_names {
        let range = match workbook.worksheet_range(sn) {
            Ok(r) => r,
            Err(_) => continue,
        };

        let mut sheet = Sheet::new(sn.clone());

        for (row_idx, row) in range.rows().enumerate() {
            for (col_idx, cell_data) in row.iter().enumerate() {
                if col_idx as u32 >= MAX_COLS {
                    break;
                }
                let cell_ref = make_cell_ref(col_idx as u32, row_idx as u32);

                let value = match cell_data {
                    Data::Empty => continue,
                    Data::String(s) => CellValue::Text(s.clone()),
                    Data::Float(f) => CellValue::Number(*f),
                    Data::Int(i) => CellValue::Number(*i as f64),
                    Data::Bool(b) => CellValue::Bool(*b),
                    Data::Error(e) => CellValue::Error(format!("{e:?}")),
                    Data::DateTime(dt) => CellValue::Text(format!("{dt}")),
                    Data::DateTimeIso(s) => CellValue::Text(s.clone()),
                    Data::DurationIso(s) => CellValue::Text(s.clone()),
                };

                sheet.cells.insert(
                    cell_ref,
                    Cell {
                        value,
                        ..Cell::default()
                    },
                );
            }
        }

        sheets.push(sheet);
    }

    if sheets.is_empty() {
        sheets.push(Sheet::new("Sheet 1"));
    }

    let now = now_iso();
    Ok(Spreadsheet {
        id: Uuid::new_v4().to_string(),
        name,
        sheets,
        created_at: now.clone(),
        updated_at: now,
    })
}

fn import_csv(file_path: &Path, delimiter: u8) -> Result<Spreadsheet, ImpForgeError> {
    let data = std::fs::read_to_string(file_path).map_err(|e| {
        ImpForgeError::filesystem("CSV_READ", format!("Cannot read CSV file: {e}"))
    })?;

    let name = file_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Imported CSV")
        .to_string();

    let mut sheet = Sheet::new("Sheet 1");

    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());

    for (row_idx, record) in reader.records().enumerate() {
        let record = match record {
            Ok(r) => r,
            Err(_) => continue,
        };
        for (col_idx, field) in record.iter().enumerate() {
            if col_idx as u32 >= MAX_COLS {
                break;
            }
            let cell_ref = make_cell_ref(col_idx as u32, row_idx as u32);
            let field = field.trim();
            if field.is_empty() {
                continue;
            }

            let value = if let Ok(n) = field.parse::<f64>() {
                CellValue::Number(n)
            } else if field.eq_ignore_ascii_case("true") {
                CellValue::Bool(true)
            } else if field.eq_ignore_ascii_case("false") {
                CellValue::Bool(false)
            } else {
                CellValue::Text(field.to_string())
            };

            sheet.cells.insert(
                cell_ref,
                Cell {
                    value,
                    ..Cell::default()
                },
            );
        }
    }

    let now = now_iso();
    Ok(Spreadsheet {
        id: Uuid::new_v4().to_string(),
        name,
        sheets: vec![sheet],
        created_at: now.clone(),
        updated_at: now,
    })
}

// ---------------------------------------------------------------------------
// Export helpers
// ---------------------------------------------------------------------------

fn export_to_xlsx(ss: &Spreadsheet, export_dir: &Path) -> Result<String, ImpForgeError> {
    let safe_name = sanitize_filename(&ss.name);
    let path = export_dir.join(format!("{safe_name}.xlsx"));

    let mut workbook = Workbook::new();

    for sheet in &ss.sheets {
        let ws = workbook.add_worksheet();
        ws.set_name(&sheet.name).map_err(|e| {
            ImpForgeError::internal("XLSX_SHEET_NAME", format!("Invalid sheet name: {e}"))
        })?;

        for (cell_ref, cell) in &sheet.cells {
            let Some((col, row)) = parse_cell_ref(cell_ref) else {
                continue;
            };
            // rust_xlsxwriter expects col as u16
            let col16 = col as u16;

            let mut fmt = Format::new();
            if cell.format.bold {
                fmt = fmt.set_bold();
            }
            if cell.format.italic {
                fmt = fmt.set_italic();
            }

            match &cell.value {
                CellValue::Number(n) => {
                    ws.write_number_with_format(row, col16, *n, &fmt)
                        .map_err(|e| {
                            ImpForgeError::internal("XLSX_WRITE", format!("Write error: {e}"))
                        })?;
                }
                CellValue::Text(s) => {
                    ws.write_string_with_format(row, col16, s, &fmt)
                        .map_err(|e| {
                            ImpForgeError::internal("XLSX_WRITE", format!("Write error: {e}"))
                        })?;
                }
                CellValue::Bool(b) => {
                    ws.write_boolean_with_format(row, col16, *b, &fmt)
                        .map_err(|e| {
                            ImpForgeError::internal("XLSX_WRITE", format!("Write error: {e}"))
                        })?;
                }
                _ => {}
            }
        }
    }

    workbook.save(&path).map_err(|e| {
        ImpForgeError::filesystem("XLSX_SAVE", format!("Cannot save XLSX file: {e}"))
    })?;

    Ok(path.to_string_lossy().to_string())
}

fn export_to_csv(ss: &Spreadsheet, export_dir: &Path) -> Result<String, ImpForgeError> {
    let safe_name = sanitize_filename(&ss.name);
    let path = export_dir.join(format!("{safe_name}.csv"));

    let sheet = ss.sheets.first().ok_or_else(|| {
        ImpForgeError::validation("NO_SHEETS", "Spreadsheet has no sheets to export")
    })?;

    // Determine grid bounds
    let (max_col, max_row) = sheet_bounds(sheet);

    let mut output = String::new();
    for row in 0..=max_row {
        let mut row_parts = Vec::new();
        for col in 0..=max_col {
            let ref_str = make_cell_ref(col, row);
            let val = resolve_string(&sheet.cells, &ref_str);
            // Escape CSV values containing commas, quotes, or newlines
            if val.contains(',') || val.contains('"') || val.contains('\n') {
                row_parts.push(format!("\"{}\"", val.replace('"', "\"\"")));
            } else {
                row_parts.push(val);
            }
        }
        output.push_str(&row_parts.join(","));
        output.push('\n');
    }

    std::fs::write(&path, &output).map_err(|e| {
        ImpForgeError::filesystem("CSV_WRITE", format!("Cannot write CSV: {e}"))
    })?;

    Ok(path.to_string_lossy().to_string())
}

fn sheet_bounds(sheet: &Sheet) -> (u32, u32) {
    let mut max_col: u32 = 0;
    let mut max_row: u32 = 0;
    for cell_ref in sheet.cells.keys() {
        if let Some((c, r)) = parse_cell_ref(cell_ref) {
            max_col = max_col.max(c);
            max_row = max_row.max(r);
        }
    }
    (max_col, max_row)
}

fn sanitize_filename(name: &str) -> String {
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string();
    if safe.is_empty() {
        "spreadsheet".to_string()
    } else {
        safe
    }
}

// ---------------------------------------------------------------------------
// Auto-EDA (statistical analysis)
// ---------------------------------------------------------------------------

fn analyze_range(sheet: &Sheet, range: &str) -> Result<AnalysisResult, ImpForgeError> {
    let (sc, sr, ec, er) = parse_range(range).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_RANGE",
            format!("Cannot parse range: '{range}'"),
        )
    })?;

    // Collect numeric columns
    let num_cols = ec - sc + 1;
    let mut columns: Vec<Vec<f64>> = vec![Vec::new(); num_cols as usize];
    let mut col_names: Vec<String> = (sc..=ec).map(col_to_letter).collect();

    // Check if first row is a header
    let first_row_is_header = {
        let mut all_text = true;
        for col in sc..=ec {
            let ref_str = make_cell_ref(col, sr);
            if let Some(cell) = sheet.cells.get(&ref_str) {
                if matches!(cell.value, CellValue::Number(_)) {
                    all_text = false;
                    break;
                }
            }
        }
        all_text && (er > sr)
    };

    let data_start_row = if first_row_is_header {
        // Use header names
        for col in sc..=ec {
            let ref_str = make_cell_ref(col, sr);
            let name = resolve_string(&sheet.cells, &ref_str);
            if !name.is_empty() {
                col_names[(col - sc) as usize] = name;
            }
        }
        sr + 1
    } else {
        sr
    };

    for row in data_start_row..=er {
        for col in sc..=ec {
            let ref_str = make_cell_ref(col, row);
            if let Some(n) = resolve_numeric(&sheet.cells, &ref_str) {
                columns[(col - sc) as usize].push(n);
            }
        }
    }

    // Compute stats for combined data
    let all_numbers: Vec<f64> = columns.iter().flat_map(|c| c.iter().copied()).collect();
    let stats = compute_range_stats(&all_numbers);

    // Detect outliers (IQR method)
    let mut outliers = Vec::new();
    for (ci, col_data) in columns.iter().enumerate() {
        if col_data.len() < 4 {
            continue;
        }
        let mut sorted = col_data.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let q1 = percentile(&sorted, 25.0);
        let q3 = percentile(&sorted, 75.0);
        let iqr = q3 - q1;
        let lower = q1 - 1.5 * iqr;
        let upper = q3 + 1.5 * iqr;

        for (ri, val) in col_data.iter().enumerate() {
            if *val < lower || *val > upper {
                let actual_row = data_start_row + ri as u32;
                outliers.push(OutlierInfo {
                    cell_ref: make_cell_ref(sc + ci as u32, actual_row),
                    value: *val,
                    reason: if *val < lower {
                        format!("Below lower fence ({lower:.2})")
                    } else {
                        format!("Above upper fence ({upper:.2})")
                    },
                });
            }
        }
    }

    // Detect trends (simple: is data mostly increasing or decreasing?)
    let mut trends = Vec::new();
    for (ci, col_data) in columns.iter().enumerate() {
        if col_data.len() < 3 {
            continue;
        }
        let increases = col_data
            .windows(2)
            .filter(|w| w[1] > w[0])
            .count();
        let total = col_data.len() - 1;
        let ratio = increases as f64 / total as f64;
        if ratio > 0.7 {
            trends.push(format!(
                "Column {} shows an upward trend ({:.0}% increasing)",
                col_names[ci],
                ratio * 100.0
            ));
        } else if ratio < 0.3 {
            trends.push(format!(
                "Column {} shows a downward trend ({:.0}% decreasing)",
                col_names[ci],
                (1.0 - ratio) * 100.0
            ));
        }
    }

    // Detect correlations (Pearson for each pair of columns)
    let mut correlations = Vec::new();
    for i in 0..columns.len() {
        for j in (i + 1)..columns.len() {
            let min_len = columns[i].len().min(columns[j].len());
            if min_len < 3 {
                continue;
            }
            let r = pearson_correlation(&columns[i][..min_len], &columns[j][..min_len]);
            if r.abs() > 0.7 {
                correlations.push(CorrelationInfo {
                    columns: (col_names[i].clone(), col_names[j].clone()),
                    coefficient: (r * 1000.0).round() / 1000.0,
                    description: if r > 0.9 {
                        "Strong positive correlation".to_string()
                    } else if r > 0.7 {
                        "Moderate positive correlation".to_string()
                    } else if r < -0.9 {
                        "Strong negative correlation".to_string()
                    } else {
                        "Moderate negative correlation".to_string()
                    },
                });
            }
        }
    }

    // Chart suggestions
    let mut suggested_charts = Vec::new();
    if columns.len() == 1 && columns[0].len() > 2 {
        suggested_charts.push(ChartSuggestion {
            chart_type: "bar".to_string(),
            reason: "Single column of numbers works well as a bar chart".to_string(),
            data_range: range.to_string(),
        });
        suggested_charts.push(ChartSuggestion {
            chart_type: "line".to_string(),
            reason: "Track values over sequence".to_string(),
            data_range: range.to_string(),
        });
    }
    if columns.len() == 2 {
        suggested_charts.push(ChartSuggestion {
            chart_type: "scatter".to_string(),
            reason: "Two numeric columns are ideal for a scatter plot".to_string(),
            data_range: range.to_string(),
        });
    }
    if columns.len() >= 2 {
        suggested_charts.push(ChartSuggestion {
            chart_type: "line".to_string(),
            reason: "Multiple series comparison via line chart".to_string(),
            data_range: range.to_string(),
        });
    }

    let total_rows = er - data_start_row + 1;
    let summary = format!(
        "Analyzed {} rows x {} columns. {} numeric values. Range: {:.2} to {:.2}, Average: {:.2}.",
        total_rows,
        num_cols,
        all_numbers.len(),
        stats.min,
        stats.max,
        stats.average,
    );

    Ok(AnalysisResult {
        summary,
        trends,
        outliers,
        correlations,
        suggested_charts,
        stats,
    })
}

fn compute_range_stats(values: &[f64]) -> RangeStats {
    if values.is_empty() {
        return RangeStats {
            count: 0,
            sum: 0.0,
            average: 0.0,
            min: 0.0,
            max: 0.0,
            std_dev: 0.0,
        };
    }

    let count = values.len();
    let sum: f64 = values.iter().sum();
    let average = sum / count as f64;
    let min = values.iter().copied().reduce(f64::min).unwrap_or(0.0);
    let max = values.iter().copied().reduce(f64::max).unwrap_or(0.0);

    let variance = if count > 1 {
        values.iter().map(|v| (v - average).powi(2)).sum::<f64>() / (count - 1) as f64
    } else {
        0.0
    };
    let std_dev = variance.sqrt();

    RangeStats {
        count,
        sum,
        average: (average * 10000.0).round() / 10000.0,
        min,
        max,
        std_dev: (std_dev * 10000.0).round() / 10000.0,
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = (p / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn pearson_correlation(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len() as f64;
    if n < 2.0 {
        return 0.0;
    }
    let mean_x = x.iter().sum::<f64>() / n;
    let mean_y = y.iter().sum::<f64>() / n;

    let mut cov = 0.0;
    let mut var_x = 0.0;
    let mut var_y = 0.0;

    for i in 0..x.len() {
        let dx = x[i] - mean_x;
        let dy = y[i] - mean_y;
        cov += dx * dy;
        var_x += dx * dx;
        var_y += dy * dy;
    }

    let denom = (var_x * var_y).sqrt();
    if denom < f64::EPSILON {
        return 0.0;
    }
    cov / denom
}

// ---------------------------------------------------------------------------
// Pivot table computation
// ---------------------------------------------------------------------------

fn compute_pivot(
    sheet: &Sheet,
    config: &PivotConfig,
) -> Result<PivotResult, ImpForgeError> {
    let (sc, sr, ec, er) = parse_range(&config.data_range).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_RANGE",
            format!("Cannot parse pivot data range: '{}'", config.data_range),
        )
    })?;

    let num_cols = (ec - sc + 1) as usize;
    if config.row_field >= num_cols
        || config.col_field >= num_cols
        || config.value_field >= num_cols
    {
        return Err(ImpForgeError::validation(
            "INVALID_FIELD",
            "Field index exceeds range width",
        ));
    }

    // Check if first row is headers
    let has_header = {
        let mut all_text = true;
        for col in sc..=ec {
            let r = make_cell_ref(col, sr);
            if let Some(cell) = sheet.cells.get(&r) {
                if matches!(cell.value, CellValue::Number(_)) {
                    all_text = false;
                    break;
                }
            }
        }
        all_text && er > sr
    };

    let data_start = if has_header { sr + 1 } else { sr };

    // Collect unique row labels and column labels
    let mut row_labels: Vec<String> = Vec::new();
    let mut col_labels: Vec<String> = Vec::new();
    // Map: (row_label, col_label) -> Vec<f64>
    let mut buckets: HashMap<(String, String), Vec<f64>> = HashMap::new();

    for row in data_start..=er {
        let row_ref = make_cell_ref(sc + config.row_field as u32, row);
        let col_ref = make_cell_ref(sc + config.col_field as u32, row);
        let val_ref = make_cell_ref(sc + config.value_field as u32, row);

        let rl = resolve_string(&sheet.cells, &row_ref);
        let cl = resolve_string(&sheet.cells, &col_ref);
        let val = resolve_numeric(&sheet.cells, &val_ref).unwrap_or(0.0);

        if !row_labels.contains(&rl) {
            row_labels.push(rl.clone());
        }
        if !col_labels.contains(&cl) {
            col_labels.push(cl.clone());
        }
        buckets.entry((rl, cl)).or_default().push(val);
    }

    // Build result rows
    let rows: Vec<PivotRow> = row_labels
        .iter()
        .map(|rl| {
            let values: Vec<Option<f64>> = col_labels
                .iter()
                .map(|cl| {
                    buckets
                        .get(&(rl.clone(), cl.clone()))
                        .map(|vals| aggregate_values(vals, config.aggregation))
                })
                .collect();
            PivotRow {
                label: rl.clone(),
                values,
            }
        })
        .collect();

    Ok(PivotResult {
        headers: col_labels,
        rows,
    })
}

fn aggregate_values(values: &[f64], agg: PivotAggregation) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    match agg {
        PivotAggregation::Sum => values.iter().sum(),
        PivotAggregation::Average => values.iter().sum::<f64>() / values.len() as f64,
        PivotAggregation::Count => values.len() as f64,
        PivotAggregation::Min => values
            .iter()
            .copied()
            .reduce(f64::min)
            .unwrap_or(0.0),
        PivotAggregation::Max => values
            .iter()
            .copied()
            .reduce(f64::max)
            .unwrap_or(0.0),
    }
}

// ---------------------------------------------------------------------------
// Chart data extraction
// ---------------------------------------------------------------------------

/// Analyze data in a range and produce a ChartConfig with extracted series data.
fn build_chart_from_range(
    sheet: &Sheet,
    range: &str,
    chart_type: ChartType,
    title: &str,
) -> Result<ChartConfig, ImpForgeError> {
    let (sc, sr, ec, er) = parse_range(range).ok_or_else(|| {
        ImpForgeError::validation("INVALID_RANGE", format!("Cannot parse chart range: '{range}'"))
    })?;

    // Check if first row is headers
    let has_header = {
        let mut all_text = true;
        for col in sc..=ec {
            let r = make_cell_ref(col, sr);
            if let Some(cell) = sheet.cells.get(&r) {
                if matches!(cell.value, CellValue::Number(_)) {
                    all_text = false;
                    break;
                }
            }
        }
        all_text && er > sr
    };

    let data_start = if has_header { sr + 1 } else { sr };

    // Check if first column is labels
    let first_col_is_labels = {
        let mut all_text = true;
        for row in data_start..=er {
            let r = make_cell_ref(sc, row);
            if let Some(cell) = sheet.cells.get(&r) {
                if matches!(cell.value, CellValue::Number(_)) {
                    all_text = false;
                    break;
                }
            }
        }
        all_text
    };

    let series_start_col = if first_col_is_labels { sc + 1 } else { sc };

    // Extract categories (labels for X axis / pie segments)
    let categories: Vec<String> = if first_col_is_labels {
        (data_start..=er)
            .map(|row| resolve_string(&sheet.cells, &make_cell_ref(sc, row)))
            .collect()
    } else {
        (data_start..=er)
            .map(|row| format!("{}", row - data_start + 1))
            .collect()
    };

    // Extract series
    let mut series = Vec::new();
    let default_colors = vec![
        "#7c3aed", "#3b82f6", "#10b981", "#f59e0b", "#ef4444",
        "#8b5cf6", "#06b6d4", "#84cc16", "#f97316", "#ec4899",
    ];

    for col in series_start_col..=ec {
        let name = if has_header {
            resolve_string(&sheet.cells, &make_cell_ref(col, sr))
        } else {
            col_to_letter(col)
        };

        let values: Vec<f64> = (data_start..=er)
            .map(|row| resolve_numeric(&sheet.cells, &make_cell_ref(col, row)).unwrap_or(0.0))
            .collect();

        series.push(ChartSeries { name, values });
    }

    let colors: Vec<String> = default_colors
        .iter()
        .take(series.len().max(categories.len()))
        .map(|s| s.to_string())
        .collect();

    Ok(ChartConfig {
        chart_type,
        title: title.to_string(),
        data_range: range.to_string(),
        labels_range: if first_col_is_labels {
            Some(format!(
                "{}:{}",
                make_cell_ref(sc, data_start),
                make_cell_ref(sc, er)
            ))
        } else {
            None
        },
        colors,
        series,
        categories,
    })
}

// ---------------------------------------------------------------------------
// Conditional formatting evaluation
// ---------------------------------------------------------------------------

/// Check if a cell meets a conditional formatting condition.
fn eval_conditional(cell: &Cell, condition: &ConditionalType) -> bool {
    match condition {
        ConditionalType::GreaterThan { value } => {
            matches!(&cell.value, CellValue::Number(n) if *n > *value)
        }
        ConditionalType::LessThan { value } => {
            matches!(&cell.value, CellValue::Number(n) if *n < *value)
        }
        ConditionalType::EqualTo { value } => {
            matches!(&cell.value, CellValue::Number(n) if (*n - *value).abs() < f64::EPSILON)
        }
        ConditionalType::Between { min, max } => {
            matches!(&cell.value, CellValue::Number(n) if *n >= *min && *n <= *max)
        }
        ConditionalType::TextContains { text } => match &cell.value {
            CellValue::Text(s) => s.to_lowercase().contains(&text.to_lowercase()),
            _ => false,
        },
        ConditionalType::IsEmpty => matches!(&cell.value, CellValue::Empty),
        ConditionalType::IsNotEmpty => !matches!(&cell.value, CellValue::Empty),
    }
}

// ---------------------------------------------------------------------------
// Agentic cell refresh
// ---------------------------------------------------------------------------

async fn refresh_agentic_cell(agent: &AgenticCell) -> Result<CellValue, ImpForgeError> {
    match &agent.agent_type {
        AgenticType::WebFetch { url } => {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| {
                    ImpForgeError::internal("HTTP_CLIENT", format!("HTTP client error: {e}"))
                })?;
            let resp = client.get(url).send().await.map_err(|e| {
                ImpForgeError::service("WEB_FETCH", format!("Failed to fetch URL: {e}"))
            })?;
            let text = resp.text().await.map_err(|e| {
                ImpForgeError::service("WEB_FETCH", format!("Failed to read response: {e}"))
            })?;
            // Return first 1000 chars to keep cell content manageable
            let truncated = if text.len() > 1000 {
                format!("{}...", &text[..1000])
            } else {
                text
            };
            Ok(CellValue::Text(truncated))
        }
        AgenticType::ApiCall {
            endpoint,
            method,
            headers,
        } => {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| {
                    ImpForgeError::internal("HTTP_CLIENT", format!("HTTP client error: {e}"))
                })?;
            let mut req = match method.to_uppercase().as_str() {
                "POST" => client.post(endpoint),
                "PUT" => client.put(endpoint),
                "DELETE" => client.delete(endpoint),
                _ => client.get(endpoint),
            };
            for (k, v) in headers {
                req = req.header(k.as_str(), v.as_str());
            }
            let resp = req.send().await.map_err(|e| {
                ImpForgeError::service("API_CALL", format!("API request failed: {e}"))
            })?;
            let text = resp.text().await.map_err(|e| {
                ImpForgeError::service("API_CALL", format!("Failed to read API response: {e}"))
            })?;
            // Try to parse as JSON number
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(n) = val.as_f64() {
                    return Ok(CellValue::Number(n));
                }
            }
            let truncated = if text.len() > 1000 {
                format!("{}...", &text[..1000])
            } else {
                text
            };
            Ok(CellValue::Text(truncated))
        }
        AgenticType::AiGenerate { prompt } => {
            let system = "You are a data assistant inside a spreadsheet. Return concise, useful data. No markdown formatting.";
            let response = ollama_generate(system, prompt, None).await?;
            // Try to parse as number
            if let Ok(n) = response.trim().parse::<f64>() {
                return Ok(CellValue::Number(n));
            }
            Ok(CellValue::Text(response))
        }
        AgenticType::FileWatch { path } => {
            let file_path = PathBuf::from(path);
            if !file_path.exists() {
                return Ok(CellValue::Error(format!("File not found: {path}")));
            }
            let content = std::fs::read_to_string(&file_path).map_err(|e| {
                ImpForgeError::filesystem("FILE_READ", format!("Cannot read watched file: {e}"))
            })?;
            let truncated = if content.len() > 1000 {
                format!("{}...", &content[..1000])
            } else {
                content
            };
            Ok(CellValue::Text(truncated))
        }
        AgenticType::Formula { expression } => {
            // The expression is evaluated as a formula without the leading '='
            Ok(CellValue::Text(expression.clone()))
        }
    }
}

// ---------------------------------------------------------------------------
// Ollama AI helpers
// ---------------------------------------------------------------------------

fn resolve_ollama_url() -> String {
    std::env::var("OLLAMA_URL")
        .or_else(|_| std::env::var("OLLAMA_HOST"))
        .unwrap_or_else(|_| "http://localhost:11434".to_string())
        .trim_end_matches('/')
        .to_string()
}

async fn ollama_generate(
    system_prompt: &str,
    user_prompt: &str,
    model: Option<&str>,
) -> Result<String, ImpForgeError> {
    let url = resolve_ollama_url();
    let model_name = model.unwrap_or("dolphin3:8b");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(AI_FORMULA_TIMEOUT_SECS))
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
                { "role": "user",   "content": user_prompt },
            ],
            "stream": false,
        }))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                ImpForgeError::service(
                    "OLLAMA_UNREACHABLE",
                    "Cannot connect to Ollama for AI formula generation",
                )
                .with_suggestion("Start Ollama with: ollama serve")
            } else if e.is_timeout() {
                ImpForgeError::service("OLLAMA_TIMEOUT", "AI formula generation timed out")
                    .with_suggestion("Try a simpler description.")
            } else {
                ImpForgeError::service(
                    "OLLAMA_REQUEST_FAILED",
                    format!("Ollama request failed: {e}"),
                )
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
        ImpForgeError::service("OLLAMA_PARSE_ERROR", format!("Failed to parse response: {e}"))
    })?;

    let content = body
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    if content.is_empty() {
        return Err(ImpForgeError::service(
            "OLLAMA_EMPTY_RESPONSE",
            "Ollama returned an empty response",
        ));
    }

    Ok(content)
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// List all spreadsheets (metadata only).
#[tauri::command]
pub async fn sheets_list() -> AppResult<Vec<SpreadsheetMeta>> {
    let dir = sheets_dir()?;
    let mut result: Vec<SpreadsheetMeta> = Vec::new();

    let entries = std::fs::read_dir(&dir).map_err(|e| {
        ImpForgeError::filesystem("DIR_READ", format!("Cannot read spreadsheets dir: {e}"))
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !name.ends_with(".json") {
            continue;
        }

        let data = match std::fs::read_to_string(&path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        let ss: Spreadsheet = match serde_json::from_str(&data) {
            Ok(s) => s,
            Err(_) => continue,
        };

        let cell_count: usize = ss.sheets.iter().map(|s| s.cells.len()).sum();
        result.push(SpreadsheetMeta {
            id: ss.id,
            name: ss.name,
            sheet_count: ss.sheets.len(),
            cell_count,
            updated_at: ss.updated_at,
        });
    }

    result.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
    Ok(result)
}

/// Create a new blank spreadsheet.
#[tauri::command]
pub async fn sheets_create(name: String) -> AppResult<Spreadsheet> {
    let dir = sheets_dir()?;
    let now = now_iso();
    let ss = Spreadsheet {
        id: Uuid::new_v4().to_string(),
        name: if name.trim().is_empty() {
            "Untitled Spreadsheet".to_string()
        } else {
            name
        },
        sheets: vec![Sheet::new("Sheet 1")],
        created_at: now.clone(),
        updated_at: now,
    };

    save_spreadsheet(&dir, &ss)?;
    log::info!("ForgeSheets: created '{}'", ss.name);
    Ok(ss)
}

/// Open an existing spreadsheet by ID.
#[tauri::command]
pub async fn sheets_open(id: String) -> AppResult<Spreadsheet> {
    let dir = sheets_dir()?;
    load_spreadsheet(&dir, &id)
}

/// Save a spreadsheet (full replace).
#[tauri::command]
pub async fn sheets_save(id: String, data: Spreadsheet) -> AppResult<()> {
    let dir = sheets_dir()?;
    let mut ss = data;
    ss.id = id;
    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;
    Ok(())
}

/// Delete a spreadsheet.
#[tauri::command]
pub async fn sheets_delete(id: String) -> AppResult<()> {
    let dir = sheets_dir()?;
    let path = spreadsheet_path(&dir, &id);
    if !path.exists() {
        return Err(ImpForgeError::filesystem(
            "SHEET_NOT_FOUND",
            format!("Spreadsheet '{id}' not found"),
        ));
    }
    std::fs::remove_file(&path).map_err(|e| {
        ImpForgeError::filesystem("DELETE_FAILED", format!("Cannot delete spreadsheet: {e}"))
    })?;
    log::info!("ForgeSheets: deleted spreadsheet '{id}'");
    Ok(())
}

/// Import a file (.xlsx, .csv, .tsv, .ods, .json) as a new spreadsheet.
#[tauri::command]
pub async fn sheets_import_file(path: String) -> AppResult<Spreadsheet> {
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(
            ImpForgeError::filesystem("FILE_NOT_FOUND", format!("File not found: {path}"))
                .with_suggestion("Check the file path and try again."),
        );
    }

    let ss = import_from_path(&file_path)?;
    let dir = sheets_dir()?;
    save_spreadsheet(&dir, &ss)?;

    log::info!(
        "ForgeSheets: imported '{}' ({} sheets, {} cells)",
        ss.name,
        ss.sheets.len(),
        ss.sheets.iter().map(|s| s.cells.len()).sum::<usize>(),
    );

    Ok(ss)
}

/// Export a spreadsheet to .xlsx or .csv.
#[tauri::command]
pub async fn sheets_export(id: String, format: String) -> AppResult<String> {
    let dir = sheets_dir()?;
    let ss = load_spreadsheet(&dir, &id)?;

    let export_dir = dirs::document_dir().unwrap_or_else(|| dir.clone());
    if !export_dir.exists() {
        std::fs::create_dir_all(&export_dir).map_err(|e| {
            ImpForgeError::filesystem(
                "EXPORT_DIR",
                format!("Cannot create export directory: {e}"),
            )
        })?;
    }

    let result_path = match format.to_lowercase().as_str() {
        "xlsx" => export_to_xlsx(&ss, &export_dir)?,
        "csv" => export_to_csv(&ss, &export_dir)?,
        other => {
            return Err(ImpForgeError::validation(
                "INVALID_EXPORT_FORMAT",
                format!("Unsupported export format: '{other}'. Use: xlsx, csv"),
            ));
        }
    };

    log::info!("ForgeSheets: exported '{}' as {format} to {result_path}", ss.name);
    Ok(result_path)
}

/// Set a single cell's value (and optionally formula/format).
#[tauri::command]
pub async fn sheets_set_cell(
    id: String,
    sheet_index: usize,
    cell_ref: String,
    value: String,
    formula: Option<String>,
    format: Option<CellFormat>,
) -> AppResult<Cell> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get_mut(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    // Validate cell reference
    if parse_cell_ref(&cell_ref).is_none() {
        return Err(ImpForgeError::validation(
            "INVALID_CELL_REF",
            format!("Invalid cell reference: '{cell_ref}'"),
        ));
    }

    // If we have a formula, evaluate it
    let (cell_value, effective_formula) = if let Some(ref f) = formula {
        if f.starts_with('=') {
            let evaluated = evaluate_formula(&sheet.cells, f);
            (evaluated, Some(f.clone()))
        } else {
            // Not a formula, treat as raw value
            (parse_value(&value), None)
        }
    } else {
        (parse_value(&value), None)
    };

    let cell = Cell {
        value: cell_value,
        formula: effective_formula,
        format: format.unwrap_or_default(),
        note: None,
    };

    sheet.cells.insert(cell_ref, cell.clone());
    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;

    Ok(cell)
}

/// Get a range of cells.
#[tauri::command]
pub async fn sheets_get_range(
    id: String,
    sheet_index: usize,
    range: String,
) -> AppResult<Vec<Vec<Cell>>> {
    let dir = sheets_dir()?;
    let ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    let (sc, sr, ec, er) = parse_range(&range).ok_or_else(|| {
        ImpForgeError::validation("INVALID_RANGE", format!("Cannot parse range: '{range}'"))
    })?;

    let mut rows = Vec::new();
    for row in sr..=er {
        let mut row_cells = Vec::new();
        for col in sc..=ec {
            let ref_str = make_cell_ref(col, row);
            let cell = sheet.cells.get(&ref_str).cloned().unwrap_or_default();
            row_cells.push(cell);
        }
        rows.push(row_cells);
    }

    Ok(rows)
}

/// Add a new sheet to an existing spreadsheet.
#[tauri::command]
pub async fn sheets_add_sheet(id: String, name: String) -> AppResult<Sheet> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet_name = if name.trim().is_empty() {
        format!("Sheet {}", ss.sheets.len() + 1)
    } else {
        name
    };

    let sheet = Sheet::new(sheet_name);
    ss.sheets.push(sheet.clone());
    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;

    Ok(sheet)
}

/// Generate a formula from natural language description using AI.
/// Research: arXiv:2510.15585 (TDD + LLM for spreadsheet formula generation)
#[tauri::command]
pub async fn sheets_ai_formula(
    description: String,
    context_cells: Option<HashMap<String, String>>,
) -> AppResult<String> {
    if description.trim().is_empty() {
        return Err(ImpForgeError::validation(
            "EMPTY_DESCRIPTION",
            "Provide a description of the formula you need",
        ));
    }

    let system_prompt = r#"You are a spreadsheet formula expert inside ForgeSheets, an AI-native spreadsheet application.
Given a natural language description of what the user wants to calculate, generate the appropriate spreadsheet formula.

Rules:
- Return ONLY the formula string starting with '='
- Use standard spreadsheet functions: SUM, AVERAGE, COUNT, MIN, MAX, IF, CONCAT, ABS, ROUND, LEN, UPPER, LOWER
- Use cell references like A1, B2, ranges like A1:A10
- No explanations, no markdown, just the formula
- If the user mentions column names, map them to letters (A, B, C, etc.)

Examples:
- "sum of column A" → =SUM(A1:A100)
- "average of B2 to B20" → =AVERAGE(B2:B20)
- "if A1 is greater than 100, show 'high', otherwise 'low'" → =IF(A1>100,"high","low")
- "profit margin for D using C as cost and D as revenue" → =((D2-C2)/D2)*100
- "concatenate first name in A1 with last name in B1" → =CONCAT(A1," ",B1)"#;

    let mut user_prompt = format!("Generate a formula for: {description}");

    if let Some(context) = context_cells {
        if !context.is_empty() {
            user_prompt.push_str("\n\nContext - current cell values:");
            for (cell_ref, val) in &context {
                user_prompt.push_str(&format!("\n  {cell_ref}: {val}"));
            }
        }
    }

    let result = ollama_generate(system_prompt, &user_prompt, None).await?;

    // Extract formula — find the first line starting with '='
    let formula = result
        .lines()
        .map(|l| l.trim())
        .find(|l| l.starts_with('='))
        .unwrap_or(&result)
        .trim()
        .to_string();

    // Clean up markdown fences if LLM wrapped it
    let formula = formula
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
        .to_string();

    Ok(formula)
}

/// Auto-EDA: analyze a range and return trends, outliers, correlations, chart suggestions.
#[tauri::command]
pub async fn sheets_ai_analyze(
    id: String,
    sheet_index: usize,
    range: String,
) -> AppResult<AnalysisResult> {
    let dir = sheets_dir()?;
    let ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    analyze_range(sheet, &range)
}

/// Evaluate a formula against a cell context. Does not persist anything.
#[tauri::command]
pub async fn sheets_evaluate_formula(
    formula: String,
    cell_context: HashMap<String, String>,
) -> AppResult<CellValue> {
    // Build a cells map from the provided context
    let mut cells: HashMap<String, Cell> = HashMap::new();
    for (ref_str, val) in &cell_context {
        cells.insert(
            ref_str.clone(),
            Cell {
                value: parse_value(val),
                ..Cell::default()
            },
        );
    }

    if !formula.starts_with('=') {
        return Err(ImpForgeError::validation(
            "INVALID_FORMULA",
            "Formula must start with '='",
        ));
    }

    Ok(evaluate_formula(&cells, &formula))
}

// ---------------------------------------------------------------------------
// Agentic Cell commands
// ---------------------------------------------------------------------------

/// Add an agentic cell that autonomously fetches/computes data.
/// Research: arXiv:2403.03636 (SheetAgent)
#[tauri::command]
pub async fn sheets_add_agentic_cell(
    id: String,
    sheet_index: usize,
    cell_ref: String,
    agent_type: AgenticType,
    config: serde_json::Value,
    refresh_interval: Option<u32>,
) -> AppResult<AgenticCell> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get_mut(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    if parse_cell_ref(&cell_ref).is_none() {
        return Err(ImpForgeError::validation(
            "INVALID_CELL_REF",
            format!("Invalid cell reference: '{cell_ref}'"),
        ));
    }

    let agent = AgenticCell {
        cell_ref: cell_ref.clone(),
        agent_type,
        config,
        refresh_interval,
        last_fetched: None,
    };

    sheet.agentic_cells.insert(cell_ref, agent.clone());
    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;

    log::info!("ForgeSheets: added agentic cell '{}'", agent.cell_ref);
    Ok(agent)
}

/// Refresh an agentic cell, fetching/computing its value.
#[tauri::command]
pub async fn sheets_refresh_agentic(
    id: String,
    sheet_index: usize,
    cell_ref: String,
) -> AppResult<CellValue> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get_mut(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    let agent = sheet.agentic_cells.get(&cell_ref).ok_or_else(|| {
        ImpForgeError::validation(
            "NO_AGENT",
            format!("No agentic cell at '{cell_ref}'"),
        )
    })?.clone();

    let value = refresh_agentic_cell(&agent).await?;

    // Update the cell value
    let cell = sheet.cells.entry(cell_ref.clone()).or_insert_with(Cell::default);
    cell.value = value.clone();

    // Update last_fetched timestamp
    if let Some(ac) = sheet.agentic_cells.get_mut(&cell_ref) {
        ac.last_fetched = Some(now_iso());
    }

    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;

    Ok(value)
}

/// List all agentic cells across all sheets of a spreadsheet.
#[tauri::command]
pub async fn sheets_list_agentic(id: String) -> AppResult<Vec<AgenticCell>> {
    let dir = sheets_dir()?;
    let ss = load_spreadsheet(&dir, &id)?;

    let mut result: Vec<AgenticCell> = Vec::new();
    for sheet in &ss.sheets {
        for agent in sheet.agentic_cells.values() {
            result.push(agent.clone());
        }
    }
    Ok(result)
}

/// Remove an agentic cell.
#[tauri::command]
pub async fn sheets_remove_agentic(
    id: String,
    sheet_index: usize,
    cell_ref: String,
) -> AppResult<()> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get_mut(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    if sheet.agentic_cells.remove(&cell_ref).is_none() {
        return Err(ImpForgeError::validation(
            "NO_AGENT",
            format!("No agentic cell at '{cell_ref}'"),
        ));
    }

    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Chart commands
// ---------------------------------------------------------------------------

/// AI-powered chart suggestion: analyzes data range and suggests optimal chart config.
#[tauri::command]
pub async fn sheets_ai_chart(
    id: String,
    sheet_index: usize,
    range: String,
    chart_type: Option<ChartType>,
    title: Option<String>,
) -> AppResult<ChartConfig> {
    let dir = sheets_dir()?;
    let ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    // If chart_type is specified, use it directly; otherwise, auto-detect
    let ct = if let Some(ct) = chart_type {
        ct
    } else {
        // Simple heuristic: count columns and rows to pick chart type
        let (sc, sr, ec, er) = parse_range(&range).ok_or_else(|| {
            ImpForgeError::validation("INVALID_RANGE", format!("Cannot parse range: '{range}'"))
        })?;
        let num_cols = ec - sc + 1;
        let num_rows = er - sr + 1;

        if num_cols == 1 && num_rows > 2 {
            ChartType::Bar
        } else if num_cols == 2 {
            ChartType::Scatter
        } else if num_rows <= 8 && num_cols == 1 {
            ChartType::Pie
        } else {
            ChartType::Line
        }
    };

    let chart_title = title.unwrap_or_else(|| "Chart".to_string());
    build_chart_from_range(sheet, &range, ct, &chart_title)
}

/// Store a chart on a sheet.
#[tauri::command]
pub async fn sheets_add_chart(
    id: String,
    sheet_index: usize,
    chart: ChartConfig,
) -> AppResult<()> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get_mut(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    sheet.charts.push(chart);
    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;
    Ok(())
}

/// Remove a chart by index.
#[tauri::command]
pub async fn sheets_remove_chart(
    id: String,
    sheet_index: usize,
    chart_index: usize,
) -> AppResult<()> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get_mut(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    if chart_index >= sheet.charts.len() {
        return Err(ImpForgeError::validation(
            "INVALID_CHART",
            format!("Chart index {chart_index} does not exist"),
        ));
    }

    sheet.charts.remove(chart_index);
    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Pivot Table commands
// ---------------------------------------------------------------------------

/// Generate a pivot table from a data range.
#[tauri::command]
pub async fn sheets_pivot(
    id: String,
    sheet_index: usize,
    data_range: String,
    row_field: usize,
    col_field: usize,
    value_field: usize,
    aggregation: PivotAggregation,
) -> AppResult<PivotResult> {
    let dir = sheets_dir()?;
    let ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    let config = PivotConfig {
        data_range,
        row_field,
        col_field,
        value_field,
        aggregation,
    };

    compute_pivot(sheet, &config)
}

// ---------------------------------------------------------------------------
// Conditional Formatting commands
// ---------------------------------------------------------------------------

/// Add a conditional formatting rule to a sheet.
#[tauri::command]
pub async fn sheets_add_conditional_format(
    id: String,
    sheet_index: usize,
    range: String,
    condition: ConditionalType,
    bg_color: Option<String>,
    text_color: Option<String>,
    bold: Option<bool>,
    italic: Option<bool>,
) -> AppResult<ConditionalFormatRule> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get_mut(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    // Validate range
    if parse_range(&range).is_none() {
        return Err(ImpForgeError::validation(
            "INVALID_RANGE",
            format!("Cannot parse range: '{range}'"),
        ));
    }

    let rule = ConditionalFormatRule {
        id: Uuid::new_v4().to_string(),
        range,
        condition,
        bg_color,
        text_color,
        bold,
        italic,
    };

    sheet.conditional_formats.push(rule.clone());
    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;

    Ok(rule)
}

/// Remove a conditional formatting rule by ID.
#[tauri::command]
pub async fn sheets_remove_conditional_format(
    id: String,
    sheet_index: usize,
    rule_id: String,
) -> AppResult<()> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get_mut(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    let initial_len = sheet.conditional_formats.len();
    sheet.conditional_formats.retain(|r| r.id != rule_id);

    if sheet.conditional_formats.len() == initial_len {
        return Err(ImpForgeError::validation(
            "RULE_NOT_FOUND",
            format!("Conditional format rule '{rule_id}' not found"),
        ));
    }

    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;
    Ok(())
}

/// Evaluate all conditional formatting rules for a sheet, returning
/// the cells that match and their applied styles.
#[tauri::command]
pub async fn sheets_eval_conditional_formats(
    id: String,
    sheet_index: usize,
) -> AppResult<HashMap<String, serde_json::Value>> {
    let dir = sheets_dir()?;
    let ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    let mut result: HashMap<String, serde_json::Value> = HashMap::new();

    for rule in &sheet.conditional_formats {
        let Some((sc, sr, ec, er)) = parse_range(&rule.range) else {
            continue;
        };
        for row in sr..=er {
            for col in sc..=ec {
                let ref_str = make_cell_ref(col, row);
                if let Some(cell) = sheet.cells.get(&ref_str) {
                    if eval_conditional(cell, &rule.condition) {
                        result.insert(
                            ref_str,
                            serde_json::json!({
                                "bg_color": rule.bg_color,
                                "text_color": rule.text_color,
                                "bold": rule.bold,
                                "italic": rule.italic,
                                "rule_id": rule.id,
                            }),
                        );
                    }
                }
            }
        }
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Data Validation commands
// ---------------------------------------------------------------------------

/// Add a data validation rule to a sheet.
#[tauri::command]
pub async fn sheets_add_data_validation(
    id: String,
    sheet_index: usize,
    range: String,
    validation: ValidationType,
    error_message: Option<String>,
) -> AppResult<DataValidationRule> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get_mut(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    if parse_range(&range).is_none() {
        return Err(ImpForgeError::validation(
            "INVALID_RANGE",
            format!("Cannot parse range: '{range}'"),
        ));
    }

    let rule = DataValidationRule {
        id: Uuid::new_v4().to_string(),
        range,
        validation,
        error_message,
    };

    sheet.data_validations.push(rule.clone());
    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;

    Ok(rule)
}

/// Remove a data validation rule by ID.
#[tauri::command]
pub async fn sheets_remove_data_validation(
    id: String,
    sheet_index: usize,
    rule_id: String,
) -> AppResult<()> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get_mut(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    let initial_len = sheet.data_validations.len();
    sheet.data_validations.retain(|r| r.id != rule_id);

    if sheet.data_validations.len() == initial_len {
        return Err(ImpForgeError::validation(
            "RULE_NOT_FOUND",
            format!("Data validation rule '{rule_id}' not found"),
        ));
    }

    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;
    Ok(())
}

/// Get all data validation rules for a cell reference.
#[tauri::command]
pub async fn sheets_get_cell_validations(
    id: String,
    sheet_index: usize,
    cell_ref: String,
) -> AppResult<Vec<DataValidationRule>> {
    let dir = sheets_dir()?;
    let ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    let Some((target_col, target_row)) = parse_cell_ref(&cell_ref) else {
        return Err(ImpForgeError::validation(
            "INVALID_CELL_REF",
            format!("Invalid cell reference: '{cell_ref}'"),
        ));
    };

    let mut matching: Vec<DataValidationRule> = Vec::new();
    for rule in &sheet.data_validations {
        if let Some((sc, sr, ec, er)) = parse_range(&rule.range) {
            if target_col >= sc
                && target_col <= ec
                && target_row >= sr
                && target_row <= er
            {
                matching.push(rule.clone());
            }
        }
    }

    Ok(matching)
}

/// Set a cell note/comment.
#[tauri::command]
pub async fn sheets_set_cell_note(
    id: String,
    sheet_index: usize,
    cell_ref: String,
    note: Option<String>,
) -> AppResult<()> {
    let dir = sheets_dir()?;
    let mut ss = load_spreadsheet(&dir, &id)?;

    let sheet = ss.sheets.get_mut(sheet_index).ok_or_else(|| {
        ImpForgeError::validation(
            "INVALID_SHEET",
            format!("Sheet index {sheet_index} does not exist"),
        )
    })?;

    if parse_cell_ref(&cell_ref).is_none() {
        return Err(ImpForgeError::validation(
            "INVALID_CELL_REF",
            format!("Invalid cell reference: '{cell_ref}'"),
        ));
    }

    let cell = sheet.cells.entry(cell_ref).or_insert_with(Cell::default);
    cell.note = note;

    ss.updated_at = now_iso();
    save_spreadsheet(&dir, &ss)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Value parsing helper
// ---------------------------------------------------------------------------

fn parse_value(s: &str) -> CellValue {
    let s = s.trim();
    if s.is_empty() {
        return CellValue::Empty;
    }
    if let Ok(n) = s.parse::<f64>() {
        return CellValue::Number(n);
    }
    if s.eq_ignore_ascii_case("true") {
        return CellValue::Bool(true);
    }
    if s.eq_ignore_ascii_case("false") {
        return CellValue::Bool(false);
    }
    CellValue::Text(s.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_col_to_letter() {
        assert_eq!(col_to_letter(0), "A");
        assert_eq!(col_to_letter(1), "B");
        assert_eq!(col_to_letter(25), "Z");
        assert_eq!(col_to_letter(26), "AA");
        assert_eq!(col_to_letter(27), "AB");
        assert_eq!(col_to_letter(701), "ZZ");
    }

    #[test]
    fn test_letter_to_col() {
        assert_eq!(letter_to_col("A"), Some(0));
        assert_eq!(letter_to_col("B"), Some(1));
        assert_eq!(letter_to_col("Z"), Some(25));
        assert_eq!(letter_to_col("AA"), Some(26));
        assert_eq!(letter_to_col("AB"), Some(27));
        assert_eq!(letter_to_col("ZZ"), Some(701));
        assert_eq!(letter_to_col(""), None);
        assert_eq!(letter_to_col("a"), None); // lowercase
    }

    #[test]
    fn test_parse_cell_ref() {
        assert_eq!(parse_cell_ref("A1"), Some((0, 0)));
        assert_eq!(parse_cell_ref("B2"), Some((1, 1)));
        assert_eq!(parse_cell_ref("Z100"), Some((25, 99)));
        assert_eq!(parse_cell_ref("AA1"), Some((26, 0)));
        assert_eq!(parse_cell_ref("A0"), None); // row 0 invalid
        assert_eq!(parse_cell_ref("1A"), None); // starts with digit
    }

    #[test]
    fn test_make_cell_ref() {
        assert_eq!(make_cell_ref(0, 0), "A1");
        assert_eq!(make_cell_ref(1, 1), "B2");
        assert_eq!(make_cell_ref(25, 99), "Z100");
        assert_eq!(make_cell_ref(26, 0), "AA1");
    }

    #[test]
    fn test_parse_range() {
        assert_eq!(parse_range("A1:B10"), Some((0, 0, 1, 9)));
        assert_eq!(parse_range("B10:A1"), Some((0, 0, 1, 9))); // auto-sort
        assert_eq!(parse_range("C3:C3"), Some((2, 2, 2, 2))); // single cell
        assert_eq!(parse_range("invalid"), None);
    }

    #[test]
    fn test_parse_value() {
        assert!(matches!(parse_value(""), CellValue::Empty));
        assert!(matches!(parse_value("42"), CellValue::Number(n) if (n - 42.0).abs() < f64::EPSILON));
        assert!(matches!(parse_value("3.14"), CellValue::Number(n) if (n - 3.14).abs() < 0.001));
        assert!(matches!(parse_value("true"), CellValue::Bool(true)));
        assert!(matches!(parse_value("FALSE"), CellValue::Bool(false)));
        assert!(matches!(parse_value("hello"), CellValue::Text(_)));
    }

    #[test]
    fn test_formula_sum() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(10.0), ..Cell::default() });
        cells.insert("A2".to_string(), Cell { value: CellValue::Number(20.0), ..Cell::default() });
        cells.insert("A3".to_string(), Cell { value: CellValue::Number(30.0), ..Cell::default() });

        let result = evaluate_formula(&cells, "=SUM(A1:A3)");
        assert!(matches!(result, CellValue::Number(n) if (n - 60.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_formula_average() {
        let mut cells = HashMap::new();
        cells.insert("B1".to_string(), Cell { value: CellValue::Number(10.0), ..Cell::default() });
        cells.insert("B2".to_string(), Cell { value: CellValue::Number(20.0), ..Cell::default() });

        let result = evaluate_formula(&cells, "=AVERAGE(B1:B2)");
        assert!(matches!(result, CellValue::Number(n) if (n - 15.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_formula_count() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(1.0), ..Cell::default() });
        cells.insert("A2".to_string(), Cell { value: CellValue::Text("hello".to_string()), ..Cell::default() });
        cells.insert("A3".to_string(), Cell { value: CellValue::Number(3.0), ..Cell::default() });

        let result = evaluate_formula(&cells, "=COUNT(A1:A3)");
        assert!(matches!(result, CellValue::Number(n) if (n - 2.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_formula_min_max() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(5.0), ..Cell::default() });
        cells.insert("A2".to_string(), Cell { value: CellValue::Number(15.0), ..Cell::default() });
        cells.insert("A3".to_string(), Cell { value: CellValue::Number(3.0), ..Cell::default() });

        let min = evaluate_formula(&cells, "=MIN(A1:A3)");
        assert!(matches!(min, CellValue::Number(n) if (n - 3.0).abs() < f64::EPSILON));

        let max = evaluate_formula(&cells, "=MAX(A1:A3)");
        assert!(matches!(max, CellValue::Number(n) if (n - 15.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_formula_if() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(15.0), ..Cell::default() });

        let result = evaluate_formula(&cells, "=IF(A1>10, \"yes\", \"no\")");
        assert!(matches!(result, CellValue::Text(ref s) if s == "yes"));

        cells.insert("A1".to_string(), Cell { value: CellValue::Number(5.0), ..Cell::default() });
        let result = evaluate_formula(&cells, "=IF(A1>10, \"yes\", \"no\")");
        assert!(matches!(result, CellValue::Text(ref s) if s == "no"));
    }

    #[test]
    fn test_formula_concat() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Text("Hello".to_string()), ..Cell::default() });
        cells.insert("B1".to_string(), Cell { value: CellValue::Text("World".to_string()), ..Cell::default() });

        let result = evaluate_formula(&cells, "=CONCAT(A1, \" \", B1)");
        assert!(matches!(result, CellValue::Text(ref s) if s == "Hello World"));
    }

    #[test]
    fn test_formula_arithmetic() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(10.0), ..Cell::default() });
        cells.insert("B1".to_string(), Cell { value: CellValue::Number(5.0), ..Cell::default() });

        let result = evaluate_formula(&cells, "=A1+B1");
        assert!(matches!(result, CellValue::Number(n) if (n - 15.0).abs() < f64::EPSILON));

        let result = evaluate_formula(&cells, "=A1*1.19");
        assert!(matches!(result, CellValue::Number(n) if (n - 11.9).abs() < 0.001));
    }

    #[test]
    fn test_formula_no_equals() {
        let cells = HashMap::new();
        let result = evaluate_formula(&cells, "SUM(A1:A3)");
        assert!(matches!(result, CellValue::Error(_)));
    }

    #[test]
    fn test_split_args() {
        let args = split_args("A1:A10, B1, \"hello, world\"");
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "A1:A10");
        assert_eq!(args[1], "B1");
        assert_eq!(args[2], "\"hello, world\"");
    }

    #[test]
    fn test_eval_condition_numeric() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(15.0), ..Cell::default() });

        assert!(eval_condition(&cells, "A1>10"));
        assert!(!eval_condition(&cells, "A1>20"));
        assert!(eval_condition(&cells, "A1>=15"));
        assert!(eval_condition(&cells, "A1<=15"));
        assert!(!eval_condition(&cells, "A1<10"));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("My Sheet!@#"), "My Sheet___");
        assert_eq!(sanitize_filename(""), "spreadsheet");
        assert_eq!(sanitize_filename("normal-file_name"), "normal-file_name");
    }

    #[test]
    fn test_compute_range_stats() {
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let stats = compute_range_stats(&values);
        assert_eq!(stats.count, 5);
        assert!((stats.sum - 150.0).abs() < f64::EPSILON);
        assert!((stats.average - 30.0).abs() < f64::EPSILON);
        assert!((stats.min - 10.0).abs() < f64::EPSILON);
        assert!((stats.max - 50.0).abs() < f64::EPSILON);
        assert!(stats.std_dev > 0.0);
    }

    #[test]
    fn test_compute_range_stats_empty() {
        let stats = compute_range_stats(&[]);
        assert_eq!(stats.count, 0);
        assert!((stats.sum - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pearson_correlation_perfect() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![2.0, 4.0, 6.0, 8.0, 10.0];
        let r = pearson_correlation(&x, &y);
        assert!((r - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_pearson_correlation_negative() {
        let x = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let y = vec![10.0, 8.0, 6.0, 4.0, 2.0];
        let r = pearson_correlation(&x, &y);
        assert!((r - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_sheet_bounds() {
        let mut sheet = Sheet::new("test");
        sheet.cells.insert("A1".to_string(), Cell::default());
        sheet.cells.insert("C5".to_string(), Cell::default());
        let (mc, mr) = sheet_bounds(&sheet);
        assert_eq!(mc, 2); // C = col index 2
        assert_eq!(mr, 4); // row 5 = index 4
    }

    #[test]
    fn test_formula_abs() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(-5.0), ..Cell::default() });
        let result = evaluate_formula(&cells, "=ABS(A1)");
        assert!(matches!(result, CellValue::Number(n) if (n - 5.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_formula_round() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(3.14159), ..Cell::default() });
        let result = evaluate_formula(&cells, "=ROUND(A1, 2)");
        assert!(matches!(result, CellValue::Number(n) if (n - 3.14).abs() < 0.001));
    }

    #[test]
    fn test_formula_len() {
        let result = evaluate_formula_inner(&HashMap::new(), "LEN(\"hello\")");
        assert!(matches!(result, CellValue::Number(n) if (n - 5.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_formula_upper_lower() {
        let result = evaluate_formula_inner(&HashMap::new(), "UPPER(\"hello\")");
        assert!(matches!(result, CellValue::Text(ref s) if s == "HELLO"));

        let result = evaluate_formula_inner(&HashMap::new(), "LOWER(\"HELLO\")");
        assert!(matches!(result, CellValue::Text(ref s) if s == "hello"));
    }

    // ---- Extended formula tests ----

    #[test]
    fn test_formula_vlookup() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Text("apple".to_string()), ..Cell::default() });
        cells.insert("B1".to_string(), Cell { value: CellValue::Number(1.0), ..Cell::default() });
        cells.insert("A2".to_string(), Cell { value: CellValue::Text("banana".to_string()), ..Cell::default() });
        cells.insert("B2".to_string(), Cell { value: CellValue::Number(2.0), ..Cell::default() });
        cells.insert("A3".to_string(), Cell { value: CellValue::Text("cherry".to_string()), ..Cell::default() });
        cells.insert("B3".to_string(), Cell { value: CellValue::Number(3.0), ..Cell::default() });

        let result = evaluate_formula(&cells, "=VLOOKUP(\"banana\", A1:B3, 2)");
        assert!(matches!(result, CellValue::Number(n) if (n - 2.0).abs() < f64::EPSILON));

        let result = evaluate_formula(&cells, "=VLOOKUP(\"grape\", A1:B3, 2)");
        assert!(matches!(result, CellValue::Error(_)));
    }

    #[test]
    fn test_formula_hlookup() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Text("x".to_string()), ..Cell::default() });
        cells.insert("B1".to_string(), Cell { value: CellValue::Text("y".to_string()), ..Cell::default() });
        cells.insert("A2".to_string(), Cell { value: CellValue::Number(10.0), ..Cell::default() });
        cells.insert("B2".to_string(), Cell { value: CellValue::Number(20.0), ..Cell::default() });

        let result = evaluate_formula(&cells, "=HLOOKUP(\"y\", A1:B2, 2)");
        assert!(matches!(result, CellValue::Number(n) if (n - 20.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_formula_index() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(1.0), ..Cell::default() });
        cells.insert("B1".to_string(), Cell { value: CellValue::Number(2.0), ..Cell::default() });
        cells.insert("A2".to_string(), Cell { value: CellValue::Number(3.0), ..Cell::default() });
        cells.insert("B2".to_string(), Cell { value: CellValue::Number(4.0), ..Cell::default() });

        let result = evaluate_formula(&cells, "=INDEX(A1:B2, 2, 2)");
        assert!(matches!(result, CellValue::Number(n) if (n - 4.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_formula_match_fn() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Text("a".to_string()), ..Cell::default() });
        cells.insert("A2".to_string(), Cell { value: CellValue::Text("b".to_string()), ..Cell::default() });
        cells.insert("A3".to_string(), Cell { value: CellValue::Text("c".to_string()), ..Cell::default() });

        let result = evaluate_formula(&cells, "=MATCH(\"b\", A1:A3)");
        assert!(matches!(result, CellValue::Number(n) if (n - 2.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_formula_sumif() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Text("yes".to_string()), ..Cell::default() });
        cells.insert("B1".to_string(), Cell { value: CellValue::Number(10.0), ..Cell::default() });
        cells.insert("A2".to_string(), Cell { value: CellValue::Text("no".to_string()), ..Cell::default() });
        cells.insert("B2".to_string(), Cell { value: CellValue::Number(20.0), ..Cell::default() });
        cells.insert("A3".to_string(), Cell { value: CellValue::Text("yes".to_string()), ..Cell::default() });
        cells.insert("B3".to_string(), Cell { value: CellValue::Number(30.0), ..Cell::default() });

        let result = evaluate_formula(&cells, "=SUMIF(A1:A3, \"yes\", B1:B3)");
        assert!(matches!(result, CellValue::Number(n) if (n - 40.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_formula_countif() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(5.0), ..Cell::default() });
        cells.insert("A2".to_string(), Cell { value: CellValue::Number(15.0), ..Cell::default() });
        cells.insert("A3".to_string(), Cell { value: CellValue::Number(25.0), ..Cell::default() });

        let result = evaluate_formula(&cells, "=COUNTIF(A1:A3, \">10\")");
        assert!(matches!(result, CellValue::Number(n) if (n - 2.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_formula_trim() {
        let result = evaluate_formula_inner(&HashMap::new(), "TRIM(\"  hello   world  \")");
        assert!(matches!(result, CellValue::Text(ref s) if s == "hello world"));
    }

    #[test]
    fn test_formula_left_right_mid() {
        let result = evaluate_formula_inner(&HashMap::new(), "LEFT(\"Hello\", 3)");
        assert!(matches!(result, CellValue::Text(ref s) if s == "Hel"));

        let result = evaluate_formula_inner(&HashMap::new(), "RIGHT(\"Hello\", 3)");
        assert!(matches!(result, CellValue::Text(ref s) if s == "llo"));

        let result = evaluate_formula_inner(&HashMap::new(), "MID(\"Hello World\", 7, 5)");
        assert!(matches!(result, CellValue::Text(ref s) if s == "World"));
    }

    #[test]
    fn test_formula_now_today() {
        let cells = HashMap::new();
        let result = evaluate_formula(&cells, "=NOW()");
        assert!(matches!(result, CellValue::Text(ref s) if s.contains('-') && s.contains(':')));

        let result = evaluate_formula(&cells, "=TODAY()");
        assert!(matches!(result, CellValue::Text(ref s) if s.contains('-') && !s.contains(':')));
    }

    #[test]
    fn test_formula_and_or_not() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(15.0), ..Cell::default() });

        let result = evaluate_formula(&cells, "=AND(A1>10, A1<20)");
        assert!(matches!(result, CellValue::Bool(true)));

        let result = evaluate_formula(&cells, "=AND(A1>10, A1>20)");
        assert!(matches!(result, CellValue::Bool(false)));

        let result = evaluate_formula(&cells, "=OR(A1>20, A1>10)");
        assert!(matches!(result, CellValue::Bool(true)));

        let result = evaluate_formula(&cells, "=NOT(A1>20)");
        assert!(matches!(result, CellValue::Bool(true)));
    }

    #[test]
    fn test_formula_iferror() {
        let cells = HashMap::new();
        let result = evaluate_formula(&cells, "=IFERROR(5, 0)");
        assert!(matches!(result, CellValue::Number(n) if (n - 5.0).abs() < f64::EPSILON));

        let result = evaluate_formula(&cells, "=IFERROR(NONEXIST(), 42)");
        assert!(matches!(result, CellValue::Number(n) if (n - 42.0).abs() < f64::EPSILON));
    }

    #[test]
    fn test_cell_values_equal_fn() {
        assert!(cell_values_equal(
            &CellValue::Number(5.0),
            &CellValue::Number(5.0)
        ));
        assert!(cell_values_equal(
            &CellValue::Text("abc".to_string()),
            &CellValue::Text("ABC".to_string())
        ));
        assert!(!cell_values_equal(
            &CellValue::Number(5.0),
            &CellValue::Number(6.0)
        ));
    }

    #[test]
    fn test_criteria_match_fn() {
        let mut cells = HashMap::new();
        cells.insert("A1".to_string(), Cell { value: CellValue::Number(15.0), ..Cell::default() });
        cells.insert("A2".to_string(), Cell { value: CellValue::Text("hello".to_string()), ..Cell::default() });

        assert!(eval_criteria_match(&cells, "A1", ">10"));
        assert!(!eval_criteria_match(&cells, "A1", ">20"));
        assert!(eval_criteria_match(&cells, "A1", "<=15"));
        assert!(eval_criteria_match(&cells, "A2", "hello"));
        assert!(!eval_criteria_match(&cells, "A2", "world"));
    }

    #[test]
    fn test_pivot_computation() {
        let mut sheet = Sheet::new("test");
        sheet.cells.insert("A1".to_string(), Cell { value: CellValue::Text("Region".to_string()), ..Cell::default() });
        sheet.cells.insert("B1".to_string(), Cell { value: CellValue::Text("Product".to_string()), ..Cell::default() });
        sheet.cells.insert("C1".to_string(), Cell { value: CellValue::Text("Sales".to_string()), ..Cell::default() });
        sheet.cells.insert("A2".to_string(), Cell { value: CellValue::Text("East".to_string()), ..Cell::default() });
        sheet.cells.insert("B2".to_string(), Cell { value: CellValue::Text("Widget".to_string()), ..Cell::default() });
        sheet.cells.insert("C2".to_string(), Cell { value: CellValue::Number(100.0), ..Cell::default() });
        sheet.cells.insert("A3".to_string(), Cell { value: CellValue::Text("West".to_string()), ..Cell::default() });
        sheet.cells.insert("B3".to_string(), Cell { value: CellValue::Text("Widget".to_string()), ..Cell::default() });
        sheet.cells.insert("C3".to_string(), Cell { value: CellValue::Number(200.0), ..Cell::default() });
        sheet.cells.insert("A4".to_string(), Cell { value: CellValue::Text("East".to_string()), ..Cell::default() });
        sheet.cells.insert("B4".to_string(), Cell { value: CellValue::Text("Gadget".to_string()), ..Cell::default() });
        sheet.cells.insert("C4".to_string(), Cell { value: CellValue::Number(150.0), ..Cell::default() });

        let config = PivotConfig {
            data_range: "A1:C4".to_string(),
            row_field: 0,
            col_field: 1,
            value_field: 2,
            aggregation: PivotAggregation::Sum,
        };

        let result = compute_pivot(&sheet, &config).expect("pivot should succeed");
        assert_eq!(result.rows.len(), 2);
        assert_eq!(result.headers.len(), 2);
    }

    #[test]
    fn test_conditional_check() {
        let cell_gt = Cell { value: CellValue::Number(15.0), ..Cell::default() };
        assert!(eval_conditional(&cell_gt, &ConditionalType::GreaterThan { value: 10.0 }));
        assert!(!eval_conditional(&cell_gt, &ConditionalType::LessThan { value: 10.0 }));
        assert!(eval_conditional(&cell_gt, &ConditionalType::Between { min: 10.0, max: 20.0 }));

        let cell_text = Cell { value: CellValue::Text("Hello World".to_string()), ..Cell::default() };
        assert!(eval_conditional(&cell_text, &ConditionalType::TextContains { text: "world".to_string() }));
        assert!(!eval_conditional(&cell_text, &ConditionalType::TextContains { text: "xyz".to_string() }));

        let cell_empty = Cell::default();
        assert!(eval_conditional(&cell_empty, &ConditionalType::IsEmpty));
        assert!(!eval_conditional(&cell_empty, &ConditionalType::IsNotEmpty));
    }

    #[test]
    fn test_build_chart() {
        let mut sheet = Sheet::new("test");
        sheet.cells.insert("A1".to_string(), Cell { value: CellValue::Text("Month".to_string()), ..Cell::default() });
        sheet.cells.insert("B1".to_string(), Cell { value: CellValue::Text("Sales".to_string()), ..Cell::default() });
        sheet.cells.insert("A2".to_string(), Cell { value: CellValue::Text("Jan".to_string()), ..Cell::default() });
        sheet.cells.insert("B2".to_string(), Cell { value: CellValue::Number(100.0), ..Cell::default() });
        sheet.cells.insert("A3".to_string(), Cell { value: CellValue::Text("Feb".to_string()), ..Cell::default() });
        sheet.cells.insert("B3".to_string(), Cell { value: CellValue::Number(200.0), ..Cell::default() });

        let chart = build_chart_from_range(&sheet, "A1:B3", ChartType::Bar, "Test Chart").expect("chart should succeed");
        assert_eq!(chart.series.len(), 1);
        assert_eq!(chart.series[0].name, "Sales");
        assert_eq!(chart.series[0].values, vec![100.0, 200.0]);
        assert_eq!(chart.categories, vec!["Jan", "Feb"]);
    }
}
