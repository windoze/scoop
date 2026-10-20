use super::*;

pub(super) fn key(cone: &ConeInput, item: &ApiItem) -> Result<String> {
    let mut parts = vec![cone.key.0.as_str(), item.name.as_str()];
    if let Some(parent) = &item.parent {
        parts.push(parent.0.as_str());
    } else {
        parts.push("package");
        parts.extend(item.package.iter().map(String::as_str));
    }
    routes::key(&parts)
}

impl SiteCatalog<'_> {
    pub(super) fn function_groups(&mut self) -> Result<()> {
        for (cone, item) in self.items.values() {
            if item.kind == ApiKind::Function {
                self.groups.insert(
                    item.key.clone(),
                    Route {
                        page: self.routes[&item.key].page.clone(),
                        anchor: Some(format!("group-{}", key(cone, item)?)),
                    },
                );
            }
        }
        Ok(())
    }

    pub(super) fn reference_groups(&mut self) -> Result<()> {
        for (_, item) in self.items.values() {
            let Some(comment) = &item.comment else {
                continue;
            };
            let links = comment.body.references.iter().map(|r| &r.target).chain(
                comment.tags.iter().flat_map(|tag| {
                    tag.target
                        .iter()
                        .chain(tag.body.references.iter().map(|r| &r.target))
                }),
            );
            for link in links {
                let LinkTarget::Symbols(symbols) = link else {
                    continue;
                };
                let keys = self.available_symbols(symbols);
                if keys.len() > 1 && self.common_group(&keys).is_none() {
                    let parts = keys.iter().map(|key| key.0.as_str()).collect::<Vec<_>>();
                    self.overloads.insert(
                        keys.clone(),
                        Route {
                            page: format!("overload/{}.html", routes::key(&parts)?),
                            anchor: None,
                        },
                    );
                }
            }
        }
        Ok(())
    }
}
