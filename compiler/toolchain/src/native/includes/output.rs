use super::*;
use crate::ToolchainError;

/// Remove compiler include observations while recording the actual lookup candidates.
pub fn strip_native_include_observations(
    bytes: &[u8],
    root: &Path,
    search: &[PathBuf],
    observed: &mut BTreeSet<PathBuf>,
) -> Result<Vec<u8>, ToolchainError> {
    transform(bytes, root, search, observed, |_| None)
}

pub(in crate::native) fn transform(
    bytes: &[u8],
    root: &Path,
    search: &[PathBuf],
    observed: &mut BTreeSet<PathBuf>,
    remap: impl Fn(&Path) -> Option<String>,
) -> Result<Vec<u8>, ToolchainError> {
    let mut result = Vec::with_capacity(bytes.len());
    let mut source = PathBuf::new();
    let mut raw = RawString::default();
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        if raw.contains_start_of_line(line) {
            result.extend_from_slice(line);
            continue;
        }
        if observe(line, &source, search, observed) {
            result.push(b'\n');
            continue;
        }
        if (line.starts_with(b"# ") || line.starts_with(b"#line "))
            && let Some(start) = line.iter().position(|byte| *byte == b'"')
        {
            let mut end = start + 1;
            let mut name = Vec::new();
            while end < line.len() && line[end] != b'"' {
                if line[end] == b'\\' {
                    end += 1;
                    if line.get(end).is_some_and(|b| (b'0'..=b'7').contains(b)) {
                        let mut value = 0u8;
                        for _ in 0..3 {
                            let Some(&digit) = line.get(end).filter(|b| (b'0'..=b'7').contains(b))
                            else {
                                break;
                            };
                            value = value.wrapping_mul(8).wrapping_add(digit - b'0');
                            end += 1;
                        }
                        name.push(value);
                        continue;
                    }
                }
                if end < line.len() {
                    name.push(line[end]);
                    end += 1;
                }
            }
            let name = std::str::from_utf8(&name).map_err(super::super::error)?;
            source = root.join(name);
            if let Some(mut logical) = remap(Path::new(name)) {
                if name.ends_with("//") {
                    logical.push('/');
                }
                result.extend_from_slice(&line[..start + 1]);
                result.extend_from_slice(
                    logical
                        .replace('\\', "\\\\")
                        .replace('"', "\\\"")
                        .as_bytes(),
                );
                result.extend_from_slice(&line[end..]);
                continue;
            }
        }
        result.extend_from_slice(line);
    }
    Ok(result)
}
