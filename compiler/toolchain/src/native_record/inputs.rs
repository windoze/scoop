use super::*;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub(crate) struct Input {
    path: HostPathCarrier,
    state: Option<State>,
}

#[derive(Clone, Debug, Eq, Serialize, Deserialize, PartialEq)]
struct State {
    resolved: HostPathCarrier,
    length: u64,
    modified: (bool, u64, u32),
}

impl Input {
    pub(crate) fn read(input: &Path) -> Result<Self, ToolchainError> {
        Ok(Self {
            path: path(input)?,
            state: state(input)?,
        })
    }

    pub(crate) fn is_current(&self) -> Result<bool, ToolchainError> {
        Ok(state(&self.path.to_path_buf().map_err(error)?)? == self.state)
    }
}

fn state(path: &Path) -> Result<Option<State>, ToolchainError> {
    let metadata = match path.metadata() {
        Ok(value) => value,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(None);
        }
        Err(source) => {
            return Err(error(format!(
                "cannot inspect {}: {source}",
                path.display()
            )));
        }
    };
    let (before, elapsed) = match metadata
        .modified()
        .map_err(error)?
        .duration_since(std::time::UNIX_EPOCH)
    {
        Ok(elapsed) => (false, elapsed),
        Err(error) => (true, error.duration()),
    };
    Ok(Some(State {
        resolved: super::path(&path.canonicalize().map_err(error)?)?,
        length: metadata.len(),
        modified: (before, elapsed.as_secs(), elapsed.subsec_nanos()),
    }))
}

/// Keep the candidates that can change a PATH lookup, including absent earlier entries.
pub(crate) fn program_candidates(
    program: &std::ffi::OsStr,
    search_path: &std::ffi::OsStr,
    inputs: &mut BTreeSet<PathBuf>,
) {
    let program = Path::new(program);
    if program.components().count() > 1 {
        inputs.insert(program.to_path_buf());
        return;
    }
    for directory in std::env::split_paths(search_path) {
        let candidate = directory.join(program);
        let selected = candidate.is_file();
        inputs.insert(candidate);
        if selected {
            break;
        }
    }
}

/// Observe include search choices and the parents of headers used by the probe.
pub(crate) fn header_candidates(stderr: &[u8], inputs: &mut BTreeSet<PathBuf>) {
    let mut search = false;
    let mut roots = Vec::new();
    for line in String::from_utf8_lossy(stderr).lines() {
        if line.contains("search starts here:") {
            search = true;
            continue;
        }
        if line == "End of search list." {
            search = false;
        }
        if search && line.starts_with(' ') {
            roots.push(PathBuf::from(
                line.trim().trim_end_matches(" (framework directory)"),
            ));
        } else if let Some(path) = line
            .strip_prefix("ignoring nonexistent directory \"")
            .and_then(|v| v.strip_suffix('"'))
        {
            roots.push(PathBuf::from(path));
        }
    }
    for header in inputs.clone() {
        if let Some(parent) = header.parent().filter(|p| !p.as_os_str().is_empty()) {
            inputs.insert(parent.to_path_buf());
        }
        for root in &roots {
            if let Ok(relative) = header.strip_prefix(root) {
                for candidate in &roots {
                    inputs.insert(candidate.join(relative));
                }
            }
        }
    }
    inputs.extend(roots);
}
