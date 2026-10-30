use super::*;

mod items;
pub(super) use items::ItemPage;

#[derive(Serialize)]
pub(super) struct LinkView {
    pub text: String,
    pub href: Option<String>,
    pub kind: String,
    pub summary: String,
}

impl LinkView {
    pub(super) fn new(text: impl Into<String>, page: &str, url: Option<String>) -> Self {
        Self {
            text: text.into(),
            href: url.map(|url| routes::relative(page, &url)),
            kind: String::new(),
            summary: String::new(),
        }
    }
}

#[derive(Serialize)]
pub(super) struct Section {
    pub title: String,
    pub links: Vec<LinkView>,
}

#[derive(Serialize, Default)]
pub(super) struct Listing {
    pub intro: String,
    pub sections: Vec<Section>,
}

impl SiteRenderer<'_, '_> {
    pub(super) fn item_link(&self, item: &ApiItem, page: &str) -> LinkView {
        LinkView {
            text: item.name.clone(),
            href: self
                .catalog
                .declaration_url(&item.key)
                .map(|url| routes::relative(page, &url)),
            kind: routes::kind(item.kind).into(),
            summary: item
                .comment
                .as_ref()
                .map(|c| markdown::summary(&c.body))
                .unwrap_or_default(),
        }
    }

    pub(super) fn frame(
        &self,
        page: &str,
        title: &str,
        cone: Option<&ConeInput>,
        content: String,
    ) -> Result<RenderedFile> {
        let cones = self
            .catalog
            .cones
            .values()
            .map(|cone| LinkView::new(&cone.name, page, Some(routes::cone(&cone.key))))
            .collect::<Vec<_>>();
        let packages = self
            .catalog
            .packages
            .iter()
            .filter(|((key, _), _)| cone.is_some_and(|cone| cone.key == *key))
            .map(|((_, package), path)| {
                LinkView::new(routes::package_name(package), page, Some(path.clone()))
            })
            .collect::<Vec<_>>();
        let html = self.template(
            "frame",
            minijinja::context! {
                title => title,
                content => content,
                home => routes::relative(page, "index.html"),
                search => routes::relative(page, "search.html"),
                style => routes::relative(page, "assets/style.css"),
                logo => routes::relative(page, "assets/logo.svg"),
                cones => minijinja::value::Serde(cones),
                packages => minijinja::value::Serde(packages),
                current => cone.map(|c| c.name.as_str()),
                root => &self.root,
                roots => routes::relative(page, "../../index.html"),
            },
        )?;
        Ok(RenderedFile {
            path: page.into(),
            contents: html.into_bytes(),
        })
    }
}
