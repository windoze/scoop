use super::*;
use serde::{Deserialize, Serialize};

mod record;
mod site;
use record::Record;

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
    sites: &[plan::Site],
    overviews: &[BTreeMap<ConeIdentity, overview::Overview>],
) -> BuildResult<(bool, Vec<PathBuf>)> {
    let tool = std::env::current_exe().map_err(error)?;
    let configuration = input_key(&(
        "root-catalogs-v1",
        request.private,
        request.target.canonical_triple(),
        tool.as_os_str().as_encoded_bytes(),
        plan::file_state(&tool)?,
    ))?;
    let mut site_keys = Vec::new();
    let mut configurations = Vec::new();
    let mut roots = Vec::new();
    for (site, overviews) in sites.iter().zip(overviews) {
        let label = (!site.prefix.is_empty()).then_some(&site.name);
        let config = input_key(&(&configuration, &site.prefix, label))?;
        site_keys.push((&site.prefix, site::keys(site, overviews, &config)?.0));
        configurations.push(config);
        if !site.prefix.is_empty() {
            roots.push(scoop_doc::RootReference {
                name: site.name.clone(),
                manifest: site.manifest.display().to_string(),
                url: format!("{}/index.html", site.prefix),
            });
        }
    }
    let roots_key = input_key(&(&configuration, &roots))?;
    let key = input_key(&(site_keys, &roots_key))?;
    let previous = record::load(&request.output)?;
    if let Some(previous) = &previous
        && previous.key == key
        && record::complete(&request.output, &previous.files)?
    {
        return Ok((false, Vec::new()));
    }
    let stage = tempfile::Builder::new()
        .prefix(".scoop-doc-")
        .tempdir_in(request.output.parent().unwrap())
        .map_err(|e| io_error(&request.output, e))?;
    let mut rendered = Vec::new();
    let mut files = Vec::new();
    let mut records = BTreeMap::new();
    for ((site, overviews), config) in sites.iter().zip(overviews).zip(&configurations) {
        let (record, changed) = site::render(
            request,
            site,
            overviews,
            config,
            previous
                .as_ref()
                .and_then(|record| record.sites.get(&site.prefix)),
            &stage.path().join(&site.prefix),
        )?;
        let prefix = |file: &str| {
            if site.prefix.is_empty() {
                file.to_owned()
            } else {
                format!("{}/{file}", site.prefix)
            }
        };
        files.extend(record.files.iter().map(|file| prefix(file)));
        rendered.extend(changed.iter().map(|file| prefix(file)));
        records.insert(site.prefix.clone(), record);
    }
    let roots = if roots.is_empty() {
        None
    } else {
        let pages = match record::reuse(
            &roots_key,
            previous.as_ref().and_then(|record| record.roots.as_ref()),
            &request.output,
            stage.path(),
        )? {
            Some(pages) => pages,
            None => record::render(
                stage.path(),
                roots_key,
                scoop_doc::render_roots(&roots).map_err(error)?,
                &mut rendered,
            )?,
        };
        files.extend(pages.files.iter().cloned());
        Some(pages)
    };
    files.sort();
    files.dedup();
    write(
        stage.path(),
        RECORD,
        &serde_json::to_vec(&Record {
            key,
            files,
            roots,
            sites: records,
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
