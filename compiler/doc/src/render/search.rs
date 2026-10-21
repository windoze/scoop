use super::*;

#[derive(Serialize)]
struct Entry {
    name: String,
    kind: String,
    scope: String,
    summary: String,
    aliases: Vec<String>,
    url: String,
}

impl SiteRenderer<'_, '_> {
    pub(super) fn search_index(&self) -> Result<RenderedFile> {
        let mut entries = Vec::new();
        for cone in self.catalog.cones.values() {
            entries.push(Entry {
                name: cone.name.clone(),
                kind: "Cone".into(),
                scope: String::new(),
                summary: String::new(),
                aliases: Vec::new(),
                url: routes::cone(&cone.key),
            });
            for binding in cone.bindings.iter().filter(|binding| binding.reexport) {
                if let Some(url) = self.catalog.declaration_url(&binding.target) {
                    entries.push(Entry {
                        name: binding.name.clone(),
                        kind: "re-export".into(),
                        scope: format!(
                            "{} · {}",
                            cone.name,
                            routes::package_name(&binding.package)
                        ),
                        summary: String::new(),
                        aliases: Vec::new(),
                        url,
                    });
                }
            }
        }
        for ((key, package), path) in &self.catalog.packages {
            entries.push(Entry {
                name: routes::package_name(package),
                kind: "package".into(),
                scope: self.catalog.cones[key].name.clone(),
                summary: String::new(),
                aliases: Vec::new(),
                url: path.clone(),
            });
        }
        for (key, (cone, item)) in &self.catalog.items {
            if matches!(
                item.kind,
                ApiKind::Parameter
                    | ApiKind::TypeParameter
                    | ApiKind::ContextParameter
                    | ApiKind::Getter
                    | ApiKind::Setter
            ) {
                continue;
            }
            let mut owners = Vec::new();
            let mut current = item.parent.as_ref();
            while let Some(parent) = current {
                let owner = self.catalog.items[parent].1;
                owners.push(owner.name.as_str());
                current = owner.parent.as_ref();
            }
            owners.reverse();
            let scope = std::iter::once(cone.name.clone())
                .chain(std::iter::once(routes::package_name(&item.package)))
                .chain(owners.into_iter().map(str::to_owned))
                .collect::<Vec<_>>()
                .join(" · ");
            entries.push(Entry {
                name: item.name.clone(),
                kind: routes::kind(item.kind).into(),
                scope,
                summary: item
                    .comment
                    .as_ref()
                    .map(|comment| markdown::summary(&comment.body))
                    .unwrap_or_default(),
                aliases: item.aliases.clone(),
                url: self.catalog.routes[key].url(),
            });
        }
        entries.sort_by(|a, b| (&a.name, &a.scope, &a.url).cmp(&(&b.name, &b.scope, &b.url)));
        let json = serde_json::to_string(&entries).map_err(RenderError::Encoding)?;
        Ok(RenderedFile {
            path: "assets/index.js".into(),
            contents: format!("globalThis.SCOOP_DOC_SEARCH={json};\n").into_bytes(),
        })
    }
}
