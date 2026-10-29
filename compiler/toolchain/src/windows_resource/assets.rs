use super::*;

pub(super) fn observe(
    output: &str,
    root: &Path,
    source_directory: &Path,
    includes: &[PathBuf],
    inputs: &mut BTreeSet<PathBuf>,
) -> Result<(), ToolchainError> {
    for path in files(output)? {
        if path.has_root() {
            inputs.insert(path);
            continue;
        }
        let mut found = false;
        for directory in [root, source_directory]
            .into_iter()
            .chain(includes.iter().map(PathBuf::as_path))
        {
            let candidate = directory.join(&path);
            found = candidate.try_exists().map_err(error)?;
            inputs.insert(candidate);
            if found {
                break;
            }
        }
        if !found {
            return Err(error(format!(
                "resource input disappeared during compilation: {}",
                path.display()
            )));
        }
    }
    Ok(())
}

// LLVM RC's verbose output includes the parsed resource nodes. String tokens
// cannot contain newlines; quoted names can contain the node's punctuation.
fn files(output: &str) -> Result<Vec<PathBuf>, ToolchainError> {
    let mut files = Vec::new();
    for line in output.lines() {
        let Some(body) = ["Bitmap (", "Cursor (", "Icon (", "HTML (", "User-defined ("]
            .iter()
            .find_map(|prefix| line.strip_prefix(prefix))
        else {
            continue;
        };
        let mut quoted = false;
        let mut tail = None;
        for (index, ch) in body.char_indices() {
            if ch == '"' {
                quoted = !quoted;
            }
            if !quoted && body[index..].starts_with("): ") {
                tail = Some(body[index + 3..].trim());
                break;
            }
        }
        let mut value = tail.ok_or_else(|| error("invalid LLVM RC resource node output"))?;
        if value.starts_with("data = ") {
            continue;
        }
        if value.starts_with("L\"") || value.starts_with("l\"") {
            value = &value[1..];
        }
        if value.starts_with('"') {
            value = value
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
                .ok_or_else(|| error("invalid LLVM RC resource filename"))?;
        }
        files.push(PathBuf::from(value));
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resources_distinguish_inline_data_and_quoted_names_from_filenames() {
        let text = "String: \"ignored.bmp\"\nBitmap (\"name ): inside\"): \"图像 (1).bmp\"\n\
                    Icon (7): L\"icons/app.ico\"\nUser-defined (type: 10, name: 8): data = 1 \"text\" \n\
                    User-defined (type: \"kind ): inside\", name: 9): \"payload ): data.bin\"\n";
        assert_eq!(
            files(text).unwrap(),
            ["图像 (1).bmp", "icons/app.ico", "payload ): data.bin"].map(PathBuf::from)
        );
    }
}
