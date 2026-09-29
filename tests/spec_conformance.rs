//! Data-driven conformance runner for the official TOON specification fixtures.
//!
//! The fixtures under `tests/spec/fixtures/{encode,decode}` are vendored
//! verbatim from the TOON specification repository (see `tests/spec/README.md`).
//! Each test case is identified by `<category>/<file>#<index>`, for example
//! `encode/objects.json#3`; the index is the position in the file's `tests`
//! array. Case names are prose and only shown for readability.
//!
//! - **Encode** cases serialize the fixture's JSON `input` with
//!   [`serde_toon::to_string_with_options`] and compare the result byte for
//!   byte with `expected`.
//! - **Decode** cases parse the TOON `input` into a [`serde_json::Value`] with
//!   [`serde_toon::from_str_with_options`] (strict unless the fixture says
//!   `"strict": false`) and compare it with `expected` under the spec's
//!   JSON-model equality (§2: numbers by value, object keys in order).
//!   `shouldError` cases must fail; error messages are never checked.
//! - **Local** cases cover §3 host-type normalization that the fixtures cannot
//!   express (NaN, infinities, `-0.0`, `u64`/`i128`/`u128` extremes).
//!
//! # Known failures
//!
//! `tests/spec/known_failures.txt` lists case ids that are allowed to fail.
//! The list can only shrink: a listed case that now passes fails the run with
//! a request to remove it, as does an id that matches no case.
//!
//! # Reports
//!
//! Set `TOON_CONFORMANCE_REPORT=1` to print per-file pass/fail counts to
//! stderr (shown even without `--nocapture`):
//!
//! ```text
//! TOON_CONFORMANCE_REPORT=1 cargo test --test spec_conformance
//! ```

use serde::Deserialize;
use serde_json::Value as Json;
use serde_toon::{DecodeOptions, Delimiter, ToonOptions};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};

const KNOWN_FAILURES: &str = "tests/spec/known_failures.txt";

// ---------------------------------------------------------------------------
// Fixture model (tests/spec/fixtures.schema.json)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Category {
    Encode,
    Decode,
    Local,
}

impl Category {
    fn as_str(self) -> &'static str {
        match self {
            Category::Encode => "encode",
            Category::Decode => "decode",
            Category::Local => "local",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureFile {
    #[allow(dead_code)]
    version: String,
    category: Category,
    #[allow(dead_code)]
    description: String,
    tests: Vec<FixtureCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FixtureCase {
    name: String,
    input: Json,
    expected: Json,
    #[serde(default)]
    should_error: bool,
    #[serde(default)]
    options: FixtureOptions,
    #[serde(default)]
    min_spec_version: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    spec_section: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    note: Option<String>,
}

/// `deny_unknown_fields` makes a fixture update that introduces a new option
/// fail loudly instead of silently running with default settings.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct FixtureOptions {
    delimiter: Option<String>,
    indent_size: Option<usize>,
    strict: Option<bool>,
}

impl FixtureOptions {
    fn encode_options(&self) -> Result<ToonOptions, String> {
        let mut opts = ToonOptions::new();
        if let Some(d) = &self.delimiter {
            opts = opts.with_delimiter(match d.as_str() {
                "," => Delimiter::Comma,
                "\t" => Delimiter::Tab,
                "|" => Delimiter::Pipe,
                other => return Err(format!("unsupported fixture delimiter {other:?}")),
            });
        }
        if let Some(n) = self.indent_size {
            opts = opts.with_indent(n);
        }
        Ok(opts)
    }

    fn decode_options(&self) -> DecodeOptions {
        // The spec's default for `strict` is true.
        let opts = if self.strict.unwrap_or(true) {
            DecodeOptions::strict()
        } else {
            DecodeOptions::lenient()
        };
        match self.indent_size {
            Some(n) => opts.with_indent_size(n),
            None => opts,
        }
    }
}

// ---------------------------------------------------------------------------
// Case identity and outcomes
// ---------------------------------------------------------------------------

/// `<category>/<file>#<index>` for fixtures, `local/<name>` for local cases.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct CaseId(String);

impl CaseId {
    fn fixture(category: Category, file: &str, index: usize) -> Self {
        CaseId(format!("{}/{file}#{index}", category.as_str()))
    }

    fn local(name: &str) -> Self {
        CaseId(format!("local/{name}"))
    }

    fn category(&self) -> &str {
        self.0.split('/').next().unwrap_or("")
    }

    /// The file part of a fixture id (`objects.json`), or `local` for local cases.
    fn group(&self) -> &str {
        match self.0.split_once('/') {
            Some(("local", _)) => "local",
            Some((_, rest)) => rest.split('#').next().unwrap_or(rest),
            None => &self.0,
        }
    }
}

impl fmt::Display for CaseId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

enum Outcome {
    Pass,
    /// A readable explanation: input, expected and actual result.
    Fail(String),
    /// Not applicable to this crate's spec version.
    Skipped(String),
}

struct CaseResult {
    id: CaseId,
    name: String,
    outcome: Outcome,
}

/// Runs `f`, turning a panic into a failure so one crashing case does not
/// hide the results of all the others.
fn guarded(f: impl FnOnce() -> Outcome) -> Outcome {
    match panic::catch_unwind(AssertUnwindSafe(f)) {
        Ok(outcome) => outcome,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<&str>()
                .map(|s| s.to_string())
                .or_else(|| payload.downcast_ref::<String>().cloned())
                .unwrap_or_else(|| "<non-string panic payload>".to_string());
            Outcome::Fail(format!("panicked: {msg}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Known failures
// ---------------------------------------------------------------------------

/// Parsed `tests/spec/known_failures.txt`.
///
/// Format: one case id per line; blank lines and lines starting with `#` are
/// ignored, and anything after the id (separated by whitespace) is a comment.
struct KnownFailures {
    ids: BTreeSet<CaseId>,
}

impl KnownFailures {
    fn load() -> Self {
        let path = manifest_dir().join(KNOWN_FAILURES);
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let mut ids = BTreeSet::new();
        for (lineno, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let id = line.split_whitespace().next().unwrap_or(line);
            assert!(
                ids.insert(CaseId(id.to_string())),
                "{KNOWN_FAILURES}:{}: duplicate entry {id}",
                lineno + 1
            );
        }
        KnownFailures { ids }
    }

    fn contains(&self, id: &CaseId) -> bool {
        self.ids.contains(id)
    }

    fn in_category<'a>(&'a self, category: Category) -> impl Iterator<Item = &'a CaseId> + 'a {
        self.ids
            .iter()
            .filter(move |id| id.category() == category.as_str())
    }
}

// ---------------------------------------------------------------------------
// Fixture loading and execution
// ---------------------------------------------------------------------------

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn fixture_files(category: Category) -> Vec<(String, FixtureFile)> {
    let dir = manifest_dir()
        .join("tests/spec/fixtures")
        .join(category.as_str());
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|entry| entry.expect("readable directory entry").path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no fixtures found in {}", dir.display());
    paths
        .into_iter()
        .map(|path| {
            let file = file_name(&path);
            let text = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
            let fixture: FixtureFile = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("invalid fixture {}: {e}", path.display()));
            assert_eq!(
                fixture.category,
                category,
                "{} has the wrong category",
                path.display()
            );
            (file, fixture)
        })
        .collect()
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .expect("UTF-8 file name")
        .to_string()
}

/// Parses `"4.1"` into `(4, 1)`.
fn parse_version(v: &str) -> (u32, u32) {
    let (major, minor) = v.split_once('.').unwrap_or((v, "0"));
    (
        major.parse().expect("numeric major version"),
        minor.parse().expect("numeric minor version"),
    )
}

fn run_fixtures(category: Category) -> Vec<CaseResult> {
    let crate_version = parse_version(serde_toon::SPEC_VERSION);
    let mut results = Vec::new();
    for (file, fixture) in fixture_files(category) {
        for (index, case) in fixture.tests.into_iter().enumerate() {
            let id = CaseId::fixture(category, &file, index);
            let outcome = match &case.min_spec_version {
                Some(min) if parse_version(min) > crate_version => Outcome::Skipped(format!(
                    "requires spec {min}, crate targets {}",
                    serde_toon::SPEC_VERSION
                )),
                _ => guarded(|| match category {
                    Category::Encode => run_encode(&case),
                    Category::Decode => run_decode(&case),
                    Category::Local => unreachable!("local cases are not fixtures"),
                }),
            };
            results.push(CaseResult {
                id,
                name: case.name,
                outcome,
            });
        }
    }
    results
}

fn run_encode(case: &FixtureCase) -> Outcome {
    let opts = match case.options.encode_options() {
        Ok(opts) => opts,
        Err(e) => return Outcome::Fail(e),
    };
    let result = serde_toon::to_string_with_options(&case.input, opts);
    let input = serde_json::to_string(&case.input).expect("JSON input re-serializes");
    match (case.should_error, result) {
        (true, Ok(got)) => Outcome::Fail(format!(
            "input:    {input}\nexpected: an error\ngot:      {got:?}"
        )),
        (true, Err(_)) => Outcome::Pass,
        (false, Err(e)) => Outcome::Fail(format!(
            "input:    {input}\nexpected: {:?}\ngot:      error: {e}",
            case.expected.as_str().unwrap_or_default()
        )),
        (false, Ok(got)) => match case.expected.as_str() {
            Some(expected) if expected == got => Outcome::Pass,
            Some(expected) => Outcome::Fail(format!(
                "input:    {input}\nexpected: {expected:?}\ngot:      {got:?}"
            )),
            None => Outcome::Fail("fixture error: encode `expected` is not a string".into()),
        },
    }
}

fn run_decode(case: &FixtureCase) -> Outcome {
    let Some(input) = case.input.as_str() else {
        return Outcome::Fail("fixture error: decode `input` is not a string".into());
    };
    let opts = case.options.decode_options();
    let result = serde_toon::from_str_with_options::<Json>(input, opts);
    let mode = if opts.is_strict() {
        "strict"
    } else {
        "lenient"
    };
    match (case.should_error, result) {
        (true, Err(_)) => Outcome::Pass,
        (true, Ok(got)) => Outcome::Fail(format!(
            "input ({mode}): {input:?}\nexpected: an error\ngot:      {got}"
        )),
        (false, Err(e)) => Outcome::Fail(format!(
            "input ({mode}): {input:?}\nexpected: {}\ngot:      error: {}",
            case.expected,
            one_line(&e.to_string())
        )),
        (false, Ok(got)) if json_model_eq(&got, &case.expected) => Outcome::Pass,
        (false, Ok(got)) => Outcome::Fail(format!(
            "input ({mode}): {input:?}\nexpected: {}\ngot:      {got}",
            case.expected
        )),
    }
}

fn one_line(s: &str) -> String {
    s.replace('\n', " | ")
}

/// JSON-model equality (spec §2): numbers compare by value (`1 == 1.0`,
/// `-0 == 0`), exactly when both are integers and as `f64` otherwise; object
/// keys compare as ordered sequences.
fn json_model_eq(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Null, Json::Null) => true,
        (Json::Bool(x), Json::Bool(y)) => x == y,
        (Json::String(x), Json::String(y)) => x == y,
        (Json::Number(x), Json::Number(y)) => match (as_integer(x), as_integer(y)) {
            (Some(i), Some(j)) => i == j,
            _ => match (x.as_f64(), y.as_f64()) {
                (Some(f), Some(g)) => f == g,
                _ => false,
            },
        },
        (Json::Array(x), Json::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| json_model_eq(a, b))
        }
        (Json::Object(x), Json::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y)
                    .all(|((ka, va), (kb, vb))| ka == kb && json_model_eq(va, vb))
        }
        _ => false,
    }
}

fn as_integer(n: &serde_json::Number) -> Option<i128> {
    n.as_i64()
        .map(i128::from)
        .or_else(|| n.as_u64().map(i128::from))
}

// ---------------------------------------------------------------------------
// Local cases: §3 host-type normalization not expressible as JSON fixtures
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct Field<T> {
    x: T,
}

/// Checks that `value` encodes to exactly `expected`.
fn encodes_to<T: serde::Serialize>(value: T, expected: &str) -> Outcome {
    match serde_toon::to_string(&value) {
        Ok(got) if got == expected => Outcome::Pass,
        Ok(got) => Outcome::Fail(format!("expected: {expected:?}\ngot:      {got:?}")),
        Err(e) => Outcome::Fail(format!("expected: {expected:?}\ngot:      error: {e}")),
    }
}

/// Checks that `value` encodes to an unquoted spec number (§4 grammar) that
/// strictly decodes back to the same `f64`. Exponent form is allowed here
/// (§2: |n| < 1e-6 or |n| >= 1e21).
fn float_round_trips(value: f64) -> Outcome {
    let text = match serde_toon::to_string(&value) {
        Ok(text) => text,
        Err(e) => return Outcome::Fail(format!("value: {value:e}\nencode error: {e}")),
    };
    if !is_spec_number(&text) {
        return Outcome::Fail(format!(
            "value: {value:e}\nencoded as {text:?}, which is not a TOON number token"
        ));
    }
    match serde_toon::from_str_with_options::<f64>(&text, DecodeOptions::strict()) {
        Ok(back) if back == value => Outcome::Pass,
        Ok(back) => Outcome::Fail(format!(
            "value: {value:e}\nencoded: {text:?}\ndecoded: {back:e}"
        )),
        Err(e) => Outcome::Fail(format!(
            "value: {value:e}\nencoded: {text:?}\ndecode error: {e}"
        )),
    }
}

/// `/^-?[0-9]+(?:\.[0-9]+)?(?:e[+-]?[0-9]+)?$/i` without forbidden leading
/// zeros (spec §4).
fn is_spec_number(s: &str) -> bool {
    let digits = s.strip_prefix('-').unwrap_or(s);
    let (mantissa, exponent) = match digits.find(['e', 'E']) {
        Some(i) => (&digits[..i], Some(&digits[i + 1..])),
        None => (digits, None),
    };
    let (int, frac) = match mantissa.split_once('.') {
        Some((int, frac)) => (int, Some(frac)),
        None => (mantissa, None),
    };
    let all_digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    let frac_ok = match frac {
        Some(f) => all_digits(f),
        None => true,
    };
    let exponent_ok = match exponent {
        Some(e) => all_digits(e.strip_prefix(['+', '-']).unwrap_or(e)),
        None => true,
    };
    all_digits(int) && !(int.len() > 1 && int.starts_with('0')) && frac_ok && exponent_ok
}

type LocalCase = (&'static str, fn() -> Outcome);

const LOCAL_CASES: &[LocalCase] = &[
    ("nan-root", || encodes_to(f64::NAN, "null")),
    ("nan-field", || encodes_to(Field { x: f64::NAN }, "x: null")),
    ("inf-root", || encodes_to(f64::INFINITY, "null")),
    ("inf-field", || {
        encodes_to(Field { x: f64::INFINITY }, "x: null")
    }),
    ("neg-inf-root", || encodes_to(f64::NEG_INFINITY, "null")),
    ("neg-inf-field", || {
        encodes_to(
            Field {
                x: f64::NEG_INFINITY,
            },
            "x: null",
        )
    }),
    ("f32-nan-field", || {
        encodes_to(Field { x: f32::NAN }, "x: null")
    }),
    ("neg-zero-root", || encodes_to(-0.0f64, "0")),
    ("neg-zero-field", || {
        encodes_to(Field { x: -0.0f64 }, "x: 0")
    }),
    ("float-1e21-round-trip", || float_round_trips(1e21)),
    ("float-1.5e300-round-trip", || float_round_trips(1.5e300)),
    ("float-1e-7-round-trip", || float_round_trips(1e-7)),
    ("float-neg-2.5e-9-round-trip", || float_round_trips(-2.5e-9)),
    ("float-f64-max-round-trip", || float_round_trips(f64::MAX)),
    ("float-min-positive-round-trip", || {
        float_round_trips(f64::MIN_POSITIVE)
    }),
    ("u64-max-root", || {
        encodes_to(u64::MAX, "18446744073709551615")
    }),
    ("u64-max-field", || {
        encodes_to(Field { x: u64::MAX }, "x: 18446744073709551615")
    }),
    ("i64-min-root", || {
        encodes_to(i64::MIN, "-9223372036854775808")
    }),
    ("i128-max-root", || {
        encodes_to(i128::MAX, "170141183460469231731687303715884105727")
    }),
    ("i128-min-root", || {
        encodes_to(i128::MIN, "-170141183460469231731687303715884105728")
    }),
    ("i128-small-field", || {
        encodes_to(Field { x: -42i128 }, "x: -42")
    }),
    ("u128-max-root", || {
        encodes_to(u128::MAX, "340282366920938463463374607431768211455")
    }),
    ("u128-max-field", || {
        encodes_to(
            Field { x: u128::MAX },
            "x: 340282366920938463463374607431768211455",
        )
    }),
];

fn run_local() -> Vec<CaseResult> {
    LOCAL_CASES
        .iter()
        .map(|&(name, run)| CaseResult {
            id: CaseId::local(name),
            name: name.to_string(),
            outcome: guarded(run),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Verdict and reporting
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Tally {
    passed: usize,
    failed: usize,
    known: usize,
    fixed: usize,
    skipped: usize,
}

fn verdict(category: Category, results: Vec<CaseResult>) {
    let known = KnownFailures::load();
    let ids: BTreeSet<&CaseId> = results.iter().map(|r| &r.id).collect();

    let mut total = Tally::default();
    let mut per_group: BTreeMap<String, Tally> = BTreeMap::new();
    let mut failures = Vec::new();
    let mut fixed = Vec::new();
    let mut skipped = Vec::new();

    for r in &results {
        let group = per_group.entry(r.id.group().to_string()).or_default();
        let listed = known.contains(&r.id);
        match (&r.outcome, listed) {
            (Outcome::Pass, false) => {
                total.passed += 1;
                group.passed += 1;
            }
            (Outcome::Pass, true) => {
                total.fixed += 1;
                group.fixed += 1;
                fixed.push(format!("  {} {}", r.id, r.name));
            }
            (Outcome::Fail(_), true) => {
                total.known += 1;
                group.known += 1;
            }
            (Outcome::Fail(detail), false) => {
                total.failed += 1;
                group.failed += 1;
                failures.push(format!(
                    "  {} {}\n{}",
                    r.id,
                    r.name,
                    indent(detail, "      ")
                ));
            }
            (Outcome::Skipped(reason), _) => {
                skipped.push(format!("  {} {}: {reason}", r.id, r.name));
                total.skipped += 1;
                group.skipped += 1;
            }
        }
    }

    let stale: Vec<String> = known
        .in_category(category)
        .filter(|id| !ids.contains(id))
        .map(|id| format!("  {id}"))
        .collect();

    if std::env::var_os("TOON_CONFORMANCE_REPORT").is_some_and(|v| v != "0") {
        print_report(category, &per_group, &total, &results, &skipped);
    }

    let summary = format!(
        "{} conformance: {} cases, {} passed, {} failed, {} known failures, {} fixed known failures, {} skipped",
        category.as_str(),
        results.len(),
        total.passed,
        total.failed,
        total.known,
        total.fixed,
        total.skipped
    );

    let mut report = String::new();
    if !failures.is_empty() {
        report.push_str(&format!(
            "\nUNEXPECTED FAILURES ({}):\n{}\n",
            failures.len(),
            failures.join("\n")
        ));
    }
    if !fixed.is_empty() {
        report.push_str(&format!(
            "\nNOW PASSING — remove from known_failures ({KNOWN_FAILURES}):\n{}\n",
            fixed.join("\n")
        ));
    }
    if !stale.is_empty() {
        report.push_str(&format!(
            "\nUNKNOWN IDS in {KNOWN_FAILURES} (no such case — remove from known_failures):\n{}\n",
            stale.join("\n")
        ));
    }
    if !report.is_empty() {
        panic!("{report}\n{summary}");
    }
}

fn indent(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|l| format!("{prefix}{l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Writes straight to the process's stderr, bypassing libtest's output
/// capture, so the report shows without `--nocapture`.
fn print_report(
    category: Category,
    per_group: &BTreeMap<String, Tally>,
    total: &Tally,
    results: &[CaseResult],
    skipped: &[String],
) {
    let mut out = String::new();
    out.push_str(&format!(
        "\n{} conformance ({} cases, spec {})\n",
        category.as_str(),
        results.len(),
        serde_toon::SPEC_VERSION
    ));
    out.push_str(&format!(
        "  {:<24} {:>6} {:>6} {:>6} {:>6} {:>6}\n",
        "file", "pass", "fail", "known", "fixed", "skip"
    ));
    let row = |name: &str, t: &Tally| {
        format!(
            "  {:<24} {:>6} {:>6} {:>6} {:>6} {:>6}\n",
            name, t.passed, t.failed, t.known, t.fixed, t.skipped
        )
    };
    for (group, tally) in per_group {
        out.push_str(&row(group, tally));
    }
    out.push_str(&row("TOTAL", total));
    for line in skipped {
        out.push_str(&format!("  skipped:{line}\n"));
    }
    let _ = std::io::stderr().write_all(out.as_bytes());
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn spec_encode_fixtures() {
    verdict(Category::Encode, run_fixtures(Category::Encode));
}

#[test]
fn spec_decode_fixtures() {
    verdict(Category::Decode, run_fixtures(Category::Decode));
}

#[test]
fn spec_local_normalization() {
    verdict(Category::Local, run_local());
}

#[test]
fn json_model_equality_helper() {
    use serde_json::json;
    assert!(json_model_eq(&json!(1), &json!(1.0)));
    assert!(json_model_eq(&json!(-0.0), &json!(0)));
    assert!(json_model_eq(&json!(u64::MAX), &json!(u64::MAX)));
    assert!(!json_model_eq(&json!(u64::MAX), &json!(u64::MAX - 1)));
    assert!(!json_model_eq(&json!(1), &json!("1")));
    assert!(!json_model_eq(
        &json!({"a": 1, "b": 2}),
        &json!({"b": 2, "a": 1})
    ));
    assert!(json_model_eq(
        &json!([{"a": [1.5]}]),
        &json!([{"a": [1.5]}])
    ));
}

#[test]
fn spec_number_helper() {
    for ok in ["0", "-0", "1e21", "1e+21", "1.5E-7", "-2.5e-9", "0.5"] {
        assert!(is_spec_number(ok), "{ok}");
    }
    for bad in ["", "-", "05", "1.", ".5", "inf", "NaN", "1e", "+1", "\"1\""] {
        assert!(!is_spec_number(bad), "{bad}");
    }
}
