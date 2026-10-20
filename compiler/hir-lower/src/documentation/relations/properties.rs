use crate::hir;

pub(super) fn property_signature(
    name: &str,
    value: &str,
    getter: hir::DeclaredVisibility,
    setter: Option<hir::DeclaredVisibility>,
) -> String {
    match setter {
        Some(setter) => format!(
            "var {name}: {value} {{ {} get; {} set }}",
            visibility_name(getter),
            visibility_name(setter)
        ),
        None => format!("val {name}: {value}"),
    }
}

fn visibility_name(visibility: hir::DeclaredVisibility) -> &'static str {
    match visibility {
        hir::DeclaredVisibility::Public => "public",
        hir::DeclaredVisibility::Protected => "protected",
        hir::DeclaredVisibility::Internal => "internal",
        hir::DeclaredVisibility::Private => "private",
    }
}

pub(super) fn imported_visibility(
    visibility: hir::DeclaredVisibilityV1,
) -> hir::DeclaredVisibility {
    match visibility {
        hir::DeclaredVisibilityV1::Public => hir::DeclaredVisibility::Public,
        hir::DeclaredVisibilityV1::Protected => hir::DeclaredVisibility::Protected,
        hir::DeclaredVisibilityV1::Internal => hir::DeclaredVisibility::Internal,
        hir::DeclaredVisibilityV1::Private => hir::DeclaredVisibility::Private,
    }
}
