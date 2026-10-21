use scoop_ast::PackageSyntax;

use super::{BuildResult, invalid};

pub(super) fn source(group: &str, requested: Option<&str>) -> BuildResult<String> {
    let text = requested.map(str::to_owned).unwrap_or_else(|| {
        group
            .split('.')
            .map(|segment| {
                let bare = format!("package {segment}\n");
                if scoop_parser::parse(&bare).is_ok() {
                    segment.to_owned()
                } else {
                    format!("`{segment}`")
                }
            })
            .collect::<Vec<_>>()
            .join(".")
    });
    let input = format!("package {text}\n");
    let ast = scoop_parser::parse(&input).map_err(|_| invalid("invalid source package"))?;
    let PackageSyntax::QualifiedPackage { path, span, .. } = &ast.package else {
        return Err(invalid("invalid source package"));
    };
    if !ast.imports.is_empty()
        || !ast.declarations.is_empty()
        || !input[span.end as usize..].trim().is_empty()
    {
        return Err(invalid(
            "--package must contain only a qualified package name",
        ));
    }
    let mut segments = path.segments();
    if !group.split('.').all(|expected| {
        segments
            .next()
            .is_some_and(|actual| actual.text == expected)
    }) {
        return Err(invalid(format!(
            "package must equal Cone group `{group}` or one of its subpackages"
        )));
    }
    Ok(text)
}
