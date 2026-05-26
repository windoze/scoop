//! `scoop package` 子命令。
//!
//! R2 sysroot reshape 期间，当前 `.cone` archive 格式不再作为可用发布能力暴露。

use std::path::PathBuf;

use miette::Result;

const ARCHIVE_UNSUPPORTED_MESSAGE: &str = "`.cone` archive packaging is temporarily unsupported while the source-only cone model is being redesigned";

/// 执行 `scoop package <cone-root> [-o <out.cone>]`，当前仅返回稳定诊断。
pub fn run(
    _input: PathBuf,
    _output: Option<PathBuf>,
    _session_options: super::FacadeSessionOptions,
) -> Result<()> {
    Err(miette::miette!(ARCHIVE_UNSUPPORTED_MESSAGE))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::tempdir;

    #[test]
    fn package_returns_stable_unsupported_diagnostic() {
        let dir = tempdir().unwrap();
        let out = dir.path().join("out").join("fixture.cone");
        let err = super::run(
            PathBuf::from("missing-pkg"),
            Some(out.clone()),
            super::super::FacadeSessionOptions::new(),
        )
        .unwrap_err();

        assert!(err.to_string().contains(super::ARCHIVE_UNSUPPORTED_MESSAGE));
        assert!(!out.exists(), "禁用后不应写出 .cone 文件");
    }

    #[test]
    fn package_diagnostic_does_not_require_existing_input() {
        let err = super::run(
            PathBuf::from("definitely-missing-pkg"),
            None,
            super::super::FacadeSessionOptions::new(),
        )
        .unwrap_err();

        assert_eq!(err.to_string(), super::ARCHIVE_UNSUPPORTED_MESSAGE);
    }
}
