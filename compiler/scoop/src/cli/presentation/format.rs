use super::super::format::{FileResult, FileStatus};
use super::*;

impl Reporter {
    pub fn formatted(&self, check: bool, success: bool, files: &[FileResult]) -> io::Result<()> {
        let mut counts = serde_json::Map::new();
        for status in [
            FileStatus::Unchanged,
            FileStatus::Formatted,
            FileStatus::NeedsFormat,
            FileStatus::Failed,
        ] {
            counts.insert(
                status.as_str().into(),
                json!(files.iter().filter(|file| file.status == status).count()),
            );
        }
        match self.0 {
            MessageFormat::Json => write_json(
                &json!({ "schema": 1, "kind": "format", "mode": if check { "check" } else { "write" }, "success": success,
                "files": files.iter().map(|file| json!({ "path": path_value(&file.path), "status": file.status.as_str() })).collect::<Vec<_>>(), "counts": counts }),
            ),
            MessageFormat::Human => {
                let mut stderr = io::stderr().lock();
                for file in files
                    .iter()
                    .filter(|file| file.status != FileStatus::Unchanged)
                {
                    writeln!(stderr, "{} {}", file.status.as_str(), file.path.display())?;
                }
                writeln!(
                    stderr,
                    "format {}: {} unchanged, {} formatted, {} need formatting, {} failed",
                    if check { "check" } else { "write" },
                    counts["unchanged"],
                    counts["formatted"],
                    counts["needs-format"],
                    counts["failed"]
                )
            }
        }
    }
}
