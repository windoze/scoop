use super::*;
use serde::{Deserialize, Serialize};

mod record;
use record::{ConePages, Record};

const RECORD: &str = ".scoop-doc.json";

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
) -> BuildResult<(bool, Vec<PathBuf>)> {
    let tool = std::env::current_exe().map_err(error)?;
    let configuration = input_key(&(
        request.private,
        request.target.canonical_triple(),
        tool.as_os_str().as_encoded_bytes(),
        plan::file_state(&tool)?,
    ))?;
    let entries = selected
        .iter()
        .map(|id| (id.to_string(), &sources[id].key))
        .collect::<Vec<_>>();
    let shared_key = input_key(&(&configuration, entries))?;
    let overview_keys = selected
        .iter()
        .map(|id| &overviews[id].key)
        .collect::<Vec<_>>();
    let key = input_key(&(&shared_key, overview_keys))?;
    let previous = record::load(&request.output)?;
    if let Some(previous) = &previous
        && previous.key == key
        && record::complete(&request.output, &previous.files)?
    {
        return Ok((false, Vec::new()));
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
    let mut rendered = Vec::new();
    let shared = match record::reuse(
        &shared_key,
        previous.as_ref().and_then(|p| p.shared.as_ref()),
        &request.output,
        stage.path(),
    )? {
        Some(pages) => pages,
        None => record::render(
            stage.path(),
            shared_key,
            renderer.render_shared().map_err(error)?,
            &mut rendered,
        )?,
    };
    let mut cones = BTreeMap::new();
    for id in selected {
        let cone = scoop_doc::ConeKey::new(*id).map_err(error)?;
        let api_key = input_key(&(
            &configuration,
            &sources[id].key,
            catalog.link_inputs(&cone).map_err(error)?,
        ))?;
        let overview_key = input_key(&(&api_key, &overviews[id].key))?;
        let old = previous.as_ref().and_then(|p| p.cones.get(&id.to_string()));
        let api = match record::reuse(&api_key, old.map(|p| &p.api), &request.output, stage.path())?
        {
            Some(pages) => pages,
            None => record::render(
                stage.path(),
                api_key,
                renderer.render_api(&cone).map_err(error)?,
                &mut rendered,
            )?,
        };
        let overview = match record::reuse(
            &overview_key,
            old.map(|p| &p.overview),
            &request.output,
            stage.path(),
        )? {
            Some(pages) => pages,
            None => {
                let home = renderer
                    .render_overview(&cone, &overviews[id].view)
                    .map_err(error)?;
                let mut pages =
                    record::render(stage.path(), overview_key, vec![home], &mut rendered)?;
                record::resources(stage.path(), &overviews[id], &mut pages)?;
                pages
            }
        };
        cones.insert(id.to_string(), ConePages { api, overview });
    }
    let mut files = shared.files.clone();
    for cone in cones.values() {
        files.extend(cone.api.files.iter().chain(&cone.overview.files).cloned());
    }
    files.sort();
    files.dedup();
    write(
        stage.path(),
        RECORD,
        &serde_json::to_vec(&Record {
            key,
            files,
            shared: Some(shared),
            cones,
        })
        .map_err(error)?,
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
    Ok((
        true,
        rendered
            .into_iter()
            .map(|path| request.output.join(path))
            .collect(),
    ))
}

fn write(root: &Path, relative: &str, bytes: &[u8]) -> BuildResult<()> {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| io_error(&path, e))?;
    std::fs::write(&path, bytes).map_err(|e| io_error(&path, e))
}
