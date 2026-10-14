use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    Initialization,
    BeforeTest,
    Body,
    AfterTest,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Location {
    pub file: String,
    pub line: i64,
    pub column: i64,
    pub function: String,
    pub r#type: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AssertionValue {
    Unavailable,
    Text { text: String },
    RenderingFailed { message: String },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum FailureDetail {
    Assertion {
        assertion: String,
        message: Option<String>,
        expected: AssertionValue,
        actual: AssertionValue,
        cause: Option<String>,
    },
    Exception {
        message: String,
    },
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Failure {
    pub phase: Phase,
    pub location: Location,
    #[serde(flatten)]
    pub detail: FailureDetail,
}

#[derive(Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Passed,
    Failed,
    #[default]
    NotRun,
}

#[derive(Debug, Default)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct CaseResult {
    pub status: Status,
    pub duration_ns: Option<u64>,
    pub failures: Vec<Failure>,
    pub interruption: Option<String>,
    pub output: Option<Output>,
}

#[derive(Debug)]
pub struct RunResult {
    pub cases: Vec<CaseResult>,
    pub complete: bool,
    pub target_failure: Option<String>,
    pub output: Option<Output>,
}

impl RunResult {
    pub fn empty(count: usize) -> Self {
        Self {
            cases: (0..count).map(|_| CaseResult::default()).collect(),
            complete: false,
            target_failure: None,
            output: None,
        }
    }

    pub fn failed(&self) -> bool {
        self.target_failure.is_some() || self.cases.iter().any(|case| case.status != Status::Passed)
    }
}
