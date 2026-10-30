use super::*;

#[derive(Serialize)]
pub struct RootReference {
    pub name: String,
    pub manifest: String,
    pub url: String,
}

/// A root chooses its own bindings; links and search stay within that catalog.
pub fn render_roots(roots: &[RootReference]) -> Result<Vec<RenderedFile>> {
    let mut templates = minijinja::Environment::new();
    templates.set_auto_escape_callback(|_| minijinja::AutoEscape::Html);
    templates.set_undefined_behavior(minijinja::UndefinedBehavior::Strict);
    templates.add_template("roots", include_str!("templates/roots.html"))?;
    let html = templates
        .get_template("roots")?
        .render(minijinja::context! {
            roots => minijinja::value::Serde(roots),
        })?;
    Ok(vec![
        RenderedFile {
            path: "index.html".into(),
            contents: html.into_bytes(),
        },
        RenderedFile {
            path: "assets/style.css".into(),
            contents: include_bytes!("assets/style.css").to_vec(),
        },
        RenderedFile {
            path: "assets/logo.svg".into(),
            contents: include_bytes!("assets/logo.svg").to_vec(),
        },
    ])
}
