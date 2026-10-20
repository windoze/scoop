use super::*;
use routes::Route;
use std::collections::BTreeSet;

mod groups;
mod links;

pub use links::ConeLinkInputs;

pub struct SiteCatalog<'a> {
    pub(super) cones: BTreeMap<ConeKey, &'a ConeInput>,
    pub(super) items: BTreeMap<SymbolKey, (&'a ConeInput, &'a ApiItem)>,
    pub(super) routes: BTreeMap<SymbolKey, Route>,
    pub(super) packages: BTreeMap<(ConeKey, Vec<String>), String>,
    pub(super) pages: BTreeMap<String, Page<'a>>,
    pub(super) groups: BTreeMap<SymbolKey, Route>,
    pub(super) overloads: BTreeMap<Vec<SymbolKey>, Route>,
    known: BTreeSet<SymbolKey>,
}

pub(super) struct Page<'a> {
    pub cone: &'a ConeInput,
    pub title: String,
    pub roots: Vec<&'a ApiItem>,
}

impl<'a> SiteCatalog<'a> {
    pub fn new(inputs: &'a [ConeInput], options: SiteOptions) -> Result<Self> {
        let mut catalog = Self {
            cones: BTreeMap::new(),
            items: BTreeMap::new(),
            routes: BTreeMap::new(),
            packages: BTreeMap::new(),
            pages: BTreeMap::new(),
            groups: BTreeMap::new(),
            overloads: BTreeMap::new(),
            known: BTreeSet::new(),
        };
        for cone in inputs {
            if catalog.cones.insert(cone.key.clone(), cone).is_some() {
                return Err(RenderError::DuplicateCone(cone.key.clone()));
            }
            for item in &cone.items {
                catalog.known.insert(item.key.clone());
                catalog.known.extend(item.alternate_keys.iter().cloned());
            }
            for item in cone.visible_items(options.document_private_items) {
                catalog.items.insert(item.key.clone(), (cone, item));
                let package = (cone.key.clone(), item.package.clone());
                if let std::collections::btree_map::Entry::Vacant(entry) =
                    catalog.packages.entry(package)
                {
                    let parts = item.package.iter().map(String::as_str).collect::<Vec<_>>();
                    let path = format!("c/{}/package/{}.html", cone.key.0, routes::key(&parts)?);
                    entry.insert(path);
                }
                if routes::owns_page(item) {
                    let path = if item.kind == ApiKind::Function {
                        format!(
                            "c/{}/function/{}.html",
                            cone.key.0,
                            groups::key(cone, item)?
                        )
                    } else {
                        format!("c/{}/item/{}.html", cone.key.0, item.key.0)
                    };
                    catalog.routes.insert(
                        item.key.clone(),
                        Route {
                            page: path.clone(),
                            anchor: Some(item.key.0.clone()),
                        },
                    );
                    catalog
                        .pages
                        .entry(path)
                        .or_insert_with(|| Page {
                            cone,
                            title: item.name.clone(),
                            roots: Vec::new(),
                        })
                        .roots
                        .push(item);
                }
            }
        }
        for (_, item) in catalog.items.values().copied().collect::<Vec<_>>() {
            catalog.member_route(item);
            for alternate in &item.alternate_keys {
                catalog
                    .routes
                    .insert(alternate.clone(), catalog.routes[&item.key].clone());
            }
            if let Some(property) = &item.property {
                for accessor in std::iter::once(&property.getter).chain(property.setter.iter()) {
                    if !accessor.hidden {
                        let page = catalog.routes[&item.key].page.clone();
                        catalog
                            .routes
                            .entry(accessor.key.clone())
                            .or_insert_with(|| Route {
                                page,
                                anchor: Some(accessor.key.0.clone()),
                            });
                    }
                }
            }
        }
        for page in catalog.pages.values_mut() {
            page.roots.sort_by_key(|item| &item.key);
        }
        for cone in inputs {
            for binding in cone.bindings.iter().filter(|binding| binding.reexport) {
                if catalog.binding_visible(binding) {
                    let parts = binding
                        .package
                        .iter()
                        .map(String::as_str)
                        .collect::<Vec<_>>();
                    let path = format!("c/{}/package/{}.html", cone.key.0, routes::key(&parts)?);
                    catalog
                        .packages
                        .entry((cone.key.clone(), binding.package.clone()))
                        .or_insert(path);
                }
            }
        }
        catalog.function_groups()?;
        catalog.reference_groups()?;
        Ok(catalog)
    }

    fn member_route(&mut self, item: &ApiItem) {
        if self.routes.contains_key(&item.key) {
            return;
        }
        let parent = self.items[item.parent.as_ref().expect("non-page items have an owner")].1;
        self.member_route(parent);
        self.routes.insert(
            item.key.clone(),
            Route {
                page: self.routes[&parent.key].page.clone(),
                anchor: Some(item.key.0.clone()),
            },
        );
    }

    pub fn declaration_url(&self, key: &SymbolKey) -> Option<String> {
        self.routes.get(key).map(Route::url)
    }

    pub fn cone_url(&self, key: &ConeKey) -> Option<String> {
        self.cones.contains_key(key).then(|| routes::cone(key))
    }

    pub(super) fn binding_visible(&self, binding: &ExportBinding) -> bool {
        !self.known.contains(&binding.target) || self.routes.contains_key(&binding.target)
    }

    pub(super) fn symbol_link(&self, symbols: &[SymbolKey]) -> Option<String> {
        let keys = self.available_symbols(symbols);
        match keys.as_slice() {
            [] => None,
            [key] => self.declaration_url(key),
            _ => self
                .common_group(&keys)
                .or_else(|| self.overloads.get(&keys))
                .map(Route::url),
        }
    }

    fn available_symbols(&self, symbols: &[SymbolKey]) -> Vec<SymbolKey> {
        symbols
            .iter()
            .filter(|key| self.routes.contains_key(*key))
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn common_group(&self, symbols: &[SymbolKey]) -> Option<&Route> {
        let first = self.groups.get(&symbols[0])?;
        symbols
            .iter()
            .all(|key| self.groups.get(key) == Some(first))
            .then_some(first)
    }
}
