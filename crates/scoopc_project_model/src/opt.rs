//! Backend-neutral optimization level requested by CLI or project manifests.

use miette::Diagnostic;
use thiserror::Error;

/// 编译优化等级（`-O*` / `--opt-level` / `Cone.toml[native-build].opt-level`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptLevel {
    O0,
    O1,
    O2,
    O3,
    Os,
    Oz,
}

impl OptLevel {
    /// Return the canonical user-facing spelling for this optimization level.
    pub fn as_str(self) -> &'static str {
        match self {
            OptLevel::O0 => "0",
            OptLevel::O1 => "1",
            OptLevel::O2 => "2",
            OptLevel::O3 => "3",
            OptLevel::Os => "s",
            OptLevel::Oz => "z",
        }
    }

    /// Whether this level enables summary-driven MIR inlining.
    pub fn enables_summary_driven_mir_inlining(self) -> bool {
        !matches!(self, OptLevel::O0)
    }

    /// Whether this level enables MIR escape analysis.
    pub fn enables_mir_escape_analysis(self) -> bool {
        !matches!(self, OptLevel::O0)
    }

    /// Parse an optimization level from CLI/manifest text.
    pub fn parse(value: &str) -> Result<Self, InvalidOptLevel> {
        let v = value.trim();
        if v.is_empty() {
            return Err(InvalidOptLevel {
                value: value.to_owned(),
            });
        }

        let v = v.to_ascii_lowercase();
        match v.as_str() {
            "0" => Ok(OptLevel::O0),
            "1" => Ok(OptLevel::O1),
            "2" => Ok(OptLevel::O2),
            "3" => Ok(OptLevel::O3),
            "s" => Ok(OptLevel::Os),
            "z" => Ok(OptLevel::Oz),
            _ => Err(InvalidOptLevel {
                value: value.to_owned(),
            }),
        }
    }

    /// Parse an optimization level from manifest integer syntax.
    pub fn from_i64(value: i64) -> Result<Self, InvalidOptLevel> {
        match value {
            0 => Ok(OptLevel::O0),
            1 => Ok(OptLevel::O1),
            2 => Ok(OptLevel::O2),
            3 => Ok(OptLevel::O3),
            _ => Err(InvalidOptLevel {
                value: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("无效的优化等级：{value}（期望 0|1|2|3|s|z）")]
#[diagnostic(code(scoop::opt::invalid_opt_level))]
pub struct InvalidOptLevel {
    pub value: String,
}
