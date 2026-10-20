use super::*;

/// Catalog facts that can change a Cone's rendered links without changing its
/// frontend input. These are render inputs, not a digest of generated pages.
#[derive(Serialize)]
pub struct ConeLinkInputs {
    navigation: Vec<(ConeKey, String)>,
    symbols: BTreeMap<SymbolKey, Option<String>>,
    groups: Vec<(Vec<SymbolKey>, Option<String>)>,
    bindings: Vec<(SymbolKey, bool)>,
    extensions: Vec<ExtensionLink>,
}

#[derive(Serialize)]
struct ExtensionLink {
    key: SymbolKey,
    owner: String,
    signature: Vec<SignatureToken>,
    url: Option<String>,
}

impl SiteCatalog<'_> {
    pub fn link_inputs(&self, key: &ConeKey) -> Result<ConeLinkInputs> {
        let cone = self
            .cones
            .get(key)
            .ok_or_else(|| RenderError::MissingCone(key.clone()))?;
        let mut inputs = ConeLinkInputs {
            navigation: self
                .cones
                .values()
                .map(|cone| (cone.key.clone(), cone.name.clone()))
                .collect(),
            symbols: BTreeMap::new(),
            groups: Vec::new(),
            bindings: Vec::new(),
            extensions: Vec::new(),
        };
        for (_, item) in self.items.values().filter(|(owner, _)| owner.key == *key) {
            let tokens = item.signature.iter().chain(item.receiver.iter()).chain(
                item.equality_requirements
                    .iter()
                    .flat_map(|r| &r.components),
            );
            for target in tokens.filter_map(|token| token.link.as_ref()) {
                inputs
                    .symbols
                    .insert(target.clone(), self.declaration_url(target));
            }
            if let Some(comment) = &item.comment {
                let links = comment.body.references.iter().map(|r| &r.target).chain(
                    comment.tags.iter().flat_map(|tag| {
                        tag.target
                            .iter()
                            .chain(tag.body.references.iter().map(|r| &r.target))
                    }),
                );
                for link in links {
                    if let LinkTarget::Symbols(symbols) = link {
                        inputs
                            .groups
                            .push((symbols.clone(), self.symbol_link(symbols)));
                    }
                }
            }
        }
        for binding in cone.bindings.iter().filter(|binding| binding.reexport) {
            inputs.symbols.insert(
                binding.target.clone(),
                self.declaration_url(&binding.target),
            );
            inputs
                .bindings
                .push((binding.target.clone(), self.binding_visible(binding)));
        }
        for (owner, item) in self.items.values() {
            let receiver = item
                .receiver
                .as_ref()
                .and_then(|receiver| receiver.link.as_ref());
            if receiver
                .and_then(|key| self.items.get(key))
                .is_some_and(|(receiver, _)| receiver.key == *key)
            {
                inputs.extensions.push(ExtensionLink {
                    key: item.key.clone(),
                    owner: owner.name.clone(),
                    signature: item.signature.clone(),
                    url: self.declaration_url(&item.key),
                });
            }
        }
        Ok(inputs)
    }
}
