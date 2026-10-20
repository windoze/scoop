use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(super) struct Route {
    pub page: String,
    pub anchor: Option<String>,
}

impl Route {
    pub(super) fn url(&self) -> String {
        match &self.anchor {
            Some(anchor) => format!("{}#{anchor}", self.page),
            None => self.page.clone(),
        }
    }
}

pub(super) fn cone(key: &ConeKey) -> String {
    format!("c/{}/index.html", key.0)
}

pub(super) fn source(cone: &ConeKey, source: &SourceKey) -> String {
    format!("c/{}/source/{}.html", cone.0, source.0)
}

pub(super) fn relative(from: &str, to: &str) -> String {
    let base = std::path::Path::new(from)
        .parent()
        .expect("site paths are relative files");
    pathdiff::diff_paths(to, base)
        .expect("both site paths are relative")
        .to_string_lossy()
        .replace('\\', "/")
}

pub(super) fn key(parts: &[&str]) -> Result<String> {
    struct Parts<'a>(&'a [&'a str]);
    impl scoop_wire::WireEncode for Parts<'_> {
        fn encode(
            &self,
            encoder: &mut scoop_wire::Encoder,
        ) -> std::result::Result<(), scoop_wire::cbor::EncodeError> {
            encoder.array(self.0.len() as u64)?;
            for part in self.0 {
                encoder.text(part)?;
            }
            Ok(())
        }
    }
    Ok(scoop_wire::domain_separated_cbor_hash("scoop-doc.route", &Parts(parts))?.to_string())
}

pub(super) fn package_name(parts: &[String]) -> String {
    if parts.is_empty() {
        "(root package)".into()
    } else {
        parts
            .iter()
            .map(|part| {
                if part.contains('.') || part.chars().any(char::is_whitespace) {
                    format!("`{part}`")
                } else {
                    part.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(".")
    }
}

pub(super) fn owns_page(item: &ApiItem) -> bool {
    matches!(
        item.kind,
        ApiKind::Class
            | ApiKind::Struct
            | ApiKind::Enum
            | ApiKind::Interface
            | ApiKind::Object
            | ApiKind::Annotation
            | ApiKind::TypeAlias
    ) || item.parent.is_none()
}

pub(super) fn kind(kind: ApiKind) -> &'static str {
    match kind {
        ApiKind::Class => "class",
        ApiKind::Struct => "struct",
        ApiKind::Enum => "enum",
        ApiKind::Interface => "interface",
        ApiKind::Object => "object",
        ApiKind::Annotation => "annotation",
        ApiKind::TypeAlias => "typealias",
        ApiKind::Function => "function",
        ApiKind::Constructor => "constructor",
        ApiKind::Property => "property",
        ApiKind::Field => "field",
        ApiKind::Variant => "variant",
        ApiKind::Parameter => "parameter",
        ApiKind::TypeParameter => "type parameter",
        ApiKind::ContextParameter => "context parameter",
        ApiKind::Getter => "getter",
        ApiKind::Setter => "setter",
    }
}
