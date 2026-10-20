use super::*;
use views::{ItemPage, LinkView, Listing, Section};

impl SiteRenderer<'_, '_> {
    /// Each Cone can be rendered independently after the complete site catalog
    /// has been assembled. The caller owns incremental decisions and I/O.
    pub fn render_cone(&self, key: &ConeKey, overview: &ConeOverview) -> Result<Vec<RenderedFile>> {
        let mut files = vec![self.render_overview(key, overview)?];
        files.extend(self.render_api(key)?);
        Ok(files)
    }

    pub fn render_overview(&self, key: &ConeKey, overview: &ConeOverview) -> Result<RenderedFile> {
        let cone = self
            .catalog
            .cones
            .get(key)
            .ok_or_else(|| RenderError::MissingCone(key.clone()))?;
        let home = routes::cone(key);
        let mut sections = Vec::new();
        let packages = self
            .catalog
            .packages
            .iter()
            .filter(|((owner, _), _)| owner == key)
            .map(|((_, package), path)| {
                LinkView::new(routes::package_name(package), &home, Some(path.clone()))
            })
            .collect();
        sections.push(Section {
            title: "Packages".into(),
            links: packages,
        });
        if !cone.sources.is_empty() {
            sections.push(Section {
                title: "Source files".into(),
                links: cone
                    .sources
                    .iter()
                    .map(|source| {
                        LinkView::new(&source.path, &home, Some(routes::source(key, &source.key)))
                    })
                    .collect(),
            });
        }
        if !overview.dependencies.is_empty() {
            sections.push(Section {
                title: "Dependencies".into(),
                links: overview
                    .dependencies
                    .iter()
                    .map(|dependency| {
                        LinkView::new(
                            &dependency.name,
                            &home,
                            self.catalog.cone_url(&dependency.key),
                        )
                    })
                    .collect(),
            });
        }
        let listing = Listing {
            intro: overview
                .readme
                .as_ref()
                .map(|readme| markdown::readme(readme, &home))
                .unwrap_or_default(),
            sections,
        };
        self.frame(
            &home,
            &cone.name,
            Some(cone),
            self.template("listing", listing)?,
        )
    }

    pub fn render_api(&self, key: &ConeKey) -> Result<Vec<RenderedFile>> {
        let cone = self
            .catalog
            .cones
            .get(key)
            .ok_or_else(|| RenderError::MissingCone(key.clone()))?;
        let mut files = Vec::new();
        for ((owner, package), path) in &self.catalog.packages {
            if owner != key {
                continue;
            }
            files.push(self.package_page(cone, package, path)?);
        }
        for (path, page) in &self.catalog.pages {
            if page.cone.key != *key {
                continue;
            }
            let body = ItemPage {
                items: page
                    .roots
                    .iter()
                    .map(|item| self.item_view(item, path))
                    .collect(),
            };
            files.push(self.frame(path, &page.title, Some(cone), self.template("items", body)?)?);
        }
        for source in &cone.sources {
            #[derive(Serialize)]
            struct SourceView<'a> {
                path: &'a str,
                lines: Vec<&'a str>,
            }
            let body = self.template(
                "source",
                SourceView {
                    path: &source.path,
                    lines: source
                        .text
                        .split('\n')
                        .map(|line| line.strip_suffix('\r').unwrap_or(line))
                        .collect(),
                },
            )?;
            files.push(self.frame(
                &routes::source(key, &source.key),
                &source.path,
                Some(cone),
                body,
            )?);
        }
        Ok(files)
    }

    fn package_page(
        &self,
        cone: &ConeInput,
        package: &[String],
        path: &str,
    ) -> Result<RenderedFile> {
        let mut items = BTreeMap::new();
        for (_, item) in self.catalog.items.values().filter(|(owner, item)| {
            owner.key == cone.key && item.package == package && item.parent.is_none()
        }) {
            let mut link = self.item_link(item, path);
            if item.kind == ApiKind::Function {
                link.href = Some(routes::relative(
                    path,
                    &self.catalog.groups[&item.key].url(),
                ));
            }
            items
                .entry((item.name.clone(), link.href.clone()))
                .or_insert(link);
        }
        let bindings = cone
            .bindings
            .iter()
            .filter(|binding| {
                binding.reexport
                    && binding.package == package
                    && self.catalog.binding_visible(binding)
            })
            .map(|binding| {
                let mut link = LinkView::new(
                    &binding.name,
                    path,
                    self.catalog.declaration_url(&binding.target),
                );
                link.kind = "re-export".into();
                link
            })
            .collect::<Vec<_>>();
        let mut sections = vec![Section {
            title: "API".into(),
            links: items.into_values().collect(),
        }];
        if !bindings.is_empty() {
            sections.push(Section {
                title: "Re-exports".into(),
                links: bindings,
            });
        }
        let content = self.template(
            "listing",
            Listing {
                intro: String::new(),
                sections,
            },
        )?;
        self.frame(path, &routes::package_name(package), Some(cone), content)
    }

    /// Shared files are replaced with the same site publication as Cone files.
    pub fn render_shared(&self) -> Result<Vec<RenderedFile>> {
        let links = self
            .catalog
            .cones
            .values()
            .map(|cone| LinkView::new(&cone.name, "index.html", Some(routes::cone(&cone.key))))
            .collect();
        let index = self.template(
            "listing",
            Listing {
                intro: String::new(),
                sections: vec![Section {
                    title: "Cones".into(),
                    links,
                }],
            },
        )?;
        let search = self.template("search", ())?;
        let mut files = vec![
            self.frame("index.html", "API documentation", None, index)?,
            self.frame("search.html", "Search", None, search)?,
            RenderedFile {
                path: "assets/style.css".into(),
                contents: include_bytes!("assets/style.css").to_vec(),
            },
            RenderedFile {
                path: "assets/search.js".into(),
                contents: include_bytes!("assets/search.js").to_vec(),
            },
            self.search_index()?,
        ];
        for (keys, route) in &self.catalog.overloads {
            let links = keys
                .iter()
                .map(|key| {
                    let (cone, item) = self.catalog.items[key];
                    let mut link = self.item_link(item, &route.page);
                    link.text = item
                        .signature
                        .iter()
                        .map(|token| token.text.as_str())
                        .collect();
                    link.summary = format!(
                        "{} · {} — {}",
                        cone.name,
                        routes::package_name(&item.package),
                        link.summary
                    );
                    link
                })
                .collect();
            let body = self.template(
                "listing",
                Listing {
                    intro: String::new(),
                    sections: vec![Section {
                        title: "Resolved declarations".into(),
                        links,
                    }],
                },
            )?;
            files.push(self.frame(&route.page, "Overload group", None, body)?);
        }
        Ok(files)
    }
}
