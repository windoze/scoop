use super::*;
use serde::{Deserialize, Serialize};

const RECORD: &str = ".scoop-doc.json";

#[derive(Deserialize, Serialize)]
struct Record {
    key: String,
    files: Vec<String>,
}

pub(super) fn lock(output: &Path) -> BuildResult<std::fs::File> {
    let parent = output
        .parent()
        .ok_or_else(|| io_error(output, "site directory has no parent"))?;
    std::fs::create_dir_all(parent).map_err(|e| io_error(parent, e))?;
    let key = input_key(&output.as_os_str().as_encoded_bytes())?;
    let path = parent.join(format!(".scoop-doc-{key}.lock"));
    let file = crate::cache::open_build_lock(&path).map_err(|e| io_error(&path, e))?;
    fs4::FileExt::lock(&file).map_err(|e| io_error(&path, e))?;
    Ok(file)
}

pub(super) fn publish(
    request: &DocumentRequest,
    sources: &BTreeMap<ConeIdentity, plan::Source>,
    selected: &BTreeSet<ConeIdentity>,
    overviews: &BTreeMap<ConeIdentity, overview::Overview>,
) -> BuildResult<bool> {
    let tool = std::env::current_exe().map_err(error)?;
    let entries = selected
        .iter()
        .map(|id| (id.to_string(), &sources[id].key, &overviews[id].key))
        .collect::<Vec<_>>();
    let key = input_key(&(
        entries,
        request.private,
        request.target.canonical_triple(),
        plan::file_state(&tool)?,
    ))?;
    let record = request.output.join(RECORD);
    match std::fs::read(&record) {
        Ok(bytes) => {
            let previous: Record =
                serde_json::from_slice(&bytes).map_err(|e| io_error(&record, e))?;
            if previous.key == key {
                let mut complete = true;
                for file in &previous.files {
                    let path = request.output.join(file);
                    if !path.try_exists().map_err(|e| io_error(&path, e))? {
                        complete = false;
                        break;
                    }
                }
                if complete {
                    return Ok(false);
                }
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(io_error(&record, e)),
    }
    let inputs = selected
        .iter()
        .map(|id| {
            let path = cache::Entry::new(&request.cache, &sources[id].key).input();
            let bytes = std::fs::read(&path).map_err(|e| io_error(&path, e))?;
            scoop_doc::ConeInput::from_json(&bytes).map_err(|e| io_error(&path, e))
        })
        .collect::<BuildResult<Vec<_>>>()?;
    let catalog = scoop_doc::SiteCatalog::new(
        &inputs,
        scoop_doc::SiteOptions {
            document_private_items: request.private,
        },
    )
    .map_err(error)?;
    let renderer = scoop_doc::SiteRenderer::new(&catalog).map_err(error)?;
    let stage = tempfile::Builder::new()
        .prefix(".scoop-doc-")
        .tempdir_in(request.output.parent().unwrap())
        .map_err(|e| io_error(&request.output, e))?;
    let mut files = Vec::new();
    for file in renderer.render_shared().map_err(error)? {
        write(stage.path(), &file.path, &file.contents)?;
        files.push(file.path);
    }
    for id in selected {
        let cone = scoop_doc::ConeKey::new(*id).map_err(error)?;
        for file in renderer
            .render_cone(&cone, &overviews[id].view)
            .map_err(error)?
        {
            write(stage.path(), &file.path, &file.contents)?;
            files.push(file.path);
        }
        for (source, destination) in &overviews[id].copies {
            let path = stage.path().join(destination);
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| io_error(&path, e))?;
            std::fs::copy(source, &path).map_err(|e| io_error(source, e))?;
            files.push(destination.clone());
        }
    }
    files.sort();
    files.dedup();
    write(
        stage.path(),
        RECORD,
        &serde_json::to_vec(&Record { key, files }).map_err(error)?,
    )?;
    if request
        .output
        .try_exists()
        .map_err(|e| io_error(&request.output, e))?
    {
        rustix::fs::renameat_with(
            rustix::fs::CWD,
            stage.path(),
            rustix::fs::CWD,
            &request.output,
            rustix::fs::RenameFlags::EXCHANGE,
        )
        .map_err(|e| io_error(&request.output, e))?;
    } else {
        std::fs::rename(stage.path(), &request.output).map_err(|e| io_error(&request.output, e))?;
    }
    Ok(true)
}

fn write(root: &Path, relative: &str, bytes: &[u8]) -> BuildResult<()> {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| io_error(&path, e))?;
    std::fs::write(&path, bytes).map_err(|e| io_error(&path, e))
}
