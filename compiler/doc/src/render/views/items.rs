use super::*;

#[derive(Serialize)]
pub(in crate::render) struct ItemPage {
    pub items: Vec<ItemView>,
}

#[derive(Serialize)]
pub(in crate::render) struct ItemView {
    anchor: String,
    group_anchor: Option<String>,
    name: String,
    kind: &'static str,
    visibility: Visibility,
    synthesized: bool,
    signature: Vec<TokenView>,
    comment: String,
    tags: Vec<TagView>,
    property: Option<PropertyView>,
    source: LinkView,
    children: Vec<ItemView>,
    related: Vec<LinkView>,
}

#[derive(Serialize)]
struct TokenView {
    text: String,
    href: Option<String>,
    unavailable: bool,
}

#[derive(Serialize)]
struct TagView {
    kind: TagKind,
    label: String,
    href: Option<String>,
    body: String,
}

#[derive(Serialize)]
struct PropertyView {
    getter: AccessorView,
    setter: Option<AccessorView>,
}

#[derive(Serialize)]
struct AccessorView {
    anchor: Option<String>,
    visibility: Visibility,
}

impl SiteRenderer<'_, '_> {
    pub(in crate::render) fn item_view(&self, item: &ApiItem, page: &str) -> ItemView {
        let cone = self.catalog.items[&item.key].0;
        let (children, related): (Vec<_>, Vec<_>) = self
            .catalog
            .items
            .values()
            .map(|(_, item)| *item)
            .filter(|candidate| candidate.parent.as_ref() == Some(&item.key))
            .partition(|candidate| self.catalog.routes[&candidate.key].page == page);
        let signature = item
            .signature
            .iter()
            .map(|token| {
                let href = token
                    .link
                    .as_ref()
                    .and_then(|key| self.catalog.declaration_url(key))
                    .map(|url| routes::relative(page, &url));
                TokenView {
                    text: token.text.clone(),
                    unavailable: token.link.is_some() && href.is_none(),
                    href,
                }
            })
            .collect();
        let source_url = format!(
            "{}#L{}",
            routes::source(&cone.key, &item.location.source),
            item.location.line
        );
        let group_anchor = self
            .catalog
            .groups
            .get(&item.key)
            .filter(|route| {
                self.catalog
                    .groups
                    .iter()
                    .find(|(_, group)| *group == *route)
                    .is_some_and(|(key, _)| *key == item.key)
            })
            .and_then(|route| route.anchor.clone());
        ItemView {
            anchor: item.key.0.clone(),
            group_anchor,
            name: item.name.clone(),
            kind: routes::kind(item.kind),
            visibility: item.visibility,
            synthesized: item.synthesized,
            signature,
            comment: item
                .comment
                .as_ref()
                .map(|comment| markdown::render(&comment.body, page, self.catalog))
                .unwrap_or_default(),
            tags: item
                .comment
                .iter()
                .flat_map(|comment| &comment.tags)
                .map(|tag| TagView {
                    kind: tag.kind,
                    label: tag.label.clone(),
                    href: tag
                        .target
                        .as_ref()
                        .and_then(|link| markdown::target(link, page, self.catalog)),
                    body: markdown::render(&tag.body, page, self.catalog),
                })
                .collect(),
            property: item.property.as_ref().map(|property| PropertyView {
                getter: self.accessor(&property.getter),
                setter: property.setter.as_ref().map(|setter| self.accessor(setter)),
            }),
            source: LinkView::new(
                format!("source · line {}", item.location.line),
                page,
                Some(source_url),
            ),
            children: children
                .into_iter()
                .map(|child| self.item_view(child, page))
                .collect(),
            related: related
                .into_iter()
                .map(|child| self.item_link(child, page))
                .collect(),
        }
    }

    fn accessor(&self, accessor: &AccessorCapability) -> AccessorView {
        AccessorView {
            anchor: (!accessor.hidden && !self.catalog.items.contains_key(&accessor.key))
                .then(|| accessor.key.0.clone()),
            visibility: accessor.visibility,
        }
    }
}
