use super::*;
use record::{ConePages, SiteRecord};

pub(super) fn render(
    request: &DocumentRequest,
    site: &plan::Site,
    overviews: &BTreeMap<ConeIdentity, overview::Overview>,
    configuration: &str,
    previous: Option<&SiteRecord>,
    stage: &Path,
) -> BuildResult<(SiteRecord, Vec<String>)> {
    let sources = &site.sources;
    let selected = &site.selected;
    let output = request.output.join(&site.prefix);
    let output = output.as_path();
    let (key, shared_key) = keys(site, overviews, configuration)?;
    if let Some(previous) = previous.filter(|record| record.key == key)
        && record::complete(output, &previous.files)?
    {
        record::copy(&previous.files, output, stage)?;
        return Ok((previous.clone(), Vec::new()));
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
    let mut renderer = scoop_doc::SiteRenderer::new(&catalog).map_err(error)?;
    if !site.prefix.is_empty() {
        renderer = renderer.with_root(site.name.clone());
    }
    let mut rendered = Vec::new();
    let shared = match record::reuse(&shared_key, previous.map(|p| &p.shared), output, stage)? {
        Some(pages) => pages,
        None => record::render(
            stage,
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
        let old = previous.and_then(|p| p.cones.get(&id.to_string()));
        let api = match record::reuse(&api_key, old.map(|p| &p.api), output, stage)? {
            Some(pages) => pages,
            None => record::render(
                stage,
                api_key,
                renderer.render_api(&cone).map_err(error)?,
                &mut rendered,
            )?,
        };
        let overview = match record::reuse(&overview_key, old.map(|p| &p.overview), output, stage)?
        {
            Some(pages) => pages,
            None => {
                let home = renderer
                    .render_overview(&cone, &overviews[id].view)
                    .map_err(error)?;
                let mut pages = record::render(stage, overview_key, vec![home], &mut rendered)?;
                record::resources(stage, &overviews[id], &mut pages)?;
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
    Ok((
        SiteRecord {
            key,
            files,
            shared,
            cones,
        },
        rendered,
    ))
}

pub(super) fn keys(
    site: &plan::Site,
    overviews: &BTreeMap<ConeIdentity, overview::Overview>,
    configuration: &str,
) -> BuildResult<(String, String)> {
    let entries = site
        .selected
        .iter()
        .map(|id| (id.to_string(), &site.sources[id].key))
        .collect::<Vec<_>>();
    let shared = input_key(&(configuration, entries))?;
    let overviews = site
        .selected
        .iter()
        .map(|id| &overviews[id].key)
        .collect::<Vec<_>>();
    Ok((input_key(&(&shared, overviews))?, shared))
}
