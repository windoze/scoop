use super::*;
use pulldown_cmark::{Event, Parser, Tag};
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

pub(super) struct Overview {
    pub key: String,
    pub view: scoop_doc::ConeOverview,
    pub copies: Vec<(PathBuf, String)>,
    pub warnings: Vec<StructuredDiagnosticV1>,
}

#[derive(Deserialize, Serialize)]
struct ParsedReadme {
    text: String,
    references: Vec<Reference>,
}

#[derive(Deserialize, Serialize)]
struct Reference {
    url: String,
    start: usize,
    end: usize,
}

pub(super) fn select(source: &plan::Source, cache: &ArtifactCacheRoot) -> BuildResult<Overview> {
    let path = source.root.join("README.md");
    let state = plan::file_state(&path)?;
    let key = input_key(&(path.as_os_str().as_encoded_bytes(), &state))?;
    let mut overview = Overview {
        key,
        view: scoop_doc::ConeOverview {
            readme: None,
            dependencies: source.dependencies.clone(),
        },
        copies: Vec::new(),
        warnings: Vec::new(),
    };
    if state.is_none() {
        return Ok(overview);
    }
    let parsed = parsed(&path, cache, &overview.key)?;
    let mut states = Vec::new();
    let mut resources = BTreeMap::new();
    let cone = scoop_doc::ConeKey::new(source.identity).map_err(error)?;
    for reference in parsed.references {
        let url = &reference.url;
        if url.starts_with(['#', '/'])
            || url
                .split(['/', '?', '#'])
                .next()
                .unwrap_or("")
                .contains(':')
        {
            continue;
        }
        let (relative, suffix) = url
            .find(['?', '#'])
            .map_or((url.as_str(), ""), |i| (&url[..i], &url[i..]));
        if relative.is_empty() {
            continue;
        }
        let input = source
            .root
            .join(decode_path(relative).map_err(|e| io_error(&path, e))?);
        let state = plan::file_state(&input)?;
        states.push((url.clone(), state.clone()));
        let destination = if state.is_some() {
            let extension = input
                .extension()
                .and_then(|value| value.to_str())
                .filter(|value| value.bytes().all(|byte| byte.is_ascii_alphanumeric()))
                .unwrap_or("bin");
            let name = input_key(&input.as_os_str().as_encoded_bytes())?;
            let destination = format!("c/{}/resources/{name}.{extension}", cone.0);
            overview.copies.push((input, destination.clone()));
            Some(format!("{destination}{suffix}"))
        } else {
            overview.warnings.push(
                StructuredDiagnosticV1::new(
                    scoop_protocol::DiagnosticSeverityV1::Warning,
                    "SCOOP_DOC_RESOURCE".into(),
                    format!("README resource {url:?} does not exist"),
                    scoop_protocol::DiagnosticOriginV1::HostPathSpan {
                        path: scoop_protocol::HostPathCarrier::from_path(&path).map_err(error)?,
                        span: scoop_protocol::ProtocolByteSpan::new(
                            reference.start as u64,
                            reference.end as u64,
                        )
                        .map_err(error)?,
                    },
                    Vec::new(),
                )
                .map_err(error)?,
            );
            None
        };
        resources.insert(reference.url, destination);
    }
    overview.key = input_key(&(&overview.key, states))?;
    overview.view.readme = Some(scoop_doc::ReadmeInput {
        text: parsed.text,
        resources,
    });
    Ok(overview)
}

fn parsed(path: &Path, cache: &ArtifactCacheRoot, key: &str) -> BuildResult<ParsedReadme> {
    let directory = cache.as_path().join("documentation-readme-v1");
    let record = directory.join(format!("{key}.json"));
    match std::fs::read(&record) {
        Ok(bytes) => return serde_json::from_slice(&bytes).map_err(|e| io_error(&record, e)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_error(&record, e)),
    }
    let text = std::fs::read_to_string(path).map_err(|e| io_error(path, e))?;
    let mut references = BTreeMap::new();
    for (event, range) in Parser::new(&text).into_offset_iter() {
        if let Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) = event {
            let url = dest_url.to_string();
            references.entry(url.clone()).or_insert(Reference {
                url,
                start: range.start,
                end: range.end,
            });
        }
    }
    let parsed = ParsedReadme {
        text,
        references: references.into_values().collect(),
    };
    std::fs::create_dir_all(&directory).map_err(|e| io_error(&directory, e))?;
    let mut output =
        tempfile::NamedTempFile::new_in(&directory).map_err(|e| io_error(&record, e))?;
    serde_json::to_writer(&mut output, &parsed).map_err(|e| io_error(&record, e))?;
    output.persist(&record).map_err(|e| io_error(&record, e))?;
    Ok(parsed)
}

fn decode_path(path: &str) -> Result<String, &'static str> {
    let mut bytes = Vec::new();
    let mut input = path.bytes();
    while let Some(byte) = input.next() {
        if byte != b'%' {
            bytes.push(byte);
            continue;
        }
        let hi = input.next().and_then(|c| (c as char).to_digit(16));
        let lo = input.next().and_then(|c| (c as char).to_digit(16));
        let (Some(hi), Some(lo)) = (hi, lo) else {
            return Err("invalid percent encoding in README resource URL");
        };
        bytes.push((hi * 16 + lo) as u8);
    }
    String::from_utf8(bytes).map_err(|_| "README resource path is not UTF-8")
}
