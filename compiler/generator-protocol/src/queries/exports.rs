use super::*;
use scoop_identity::{BindingNamespace, CanonicalIdentifier};

pub(super) fn lookup(
    queries: &mut dyn hir::SemanticQueries,
    context: hir::SemanticContext,
    provider: Reference<ConeIdentity>,
    path: Vec<String>,
    namespace: ExportNamespace,
) -> Result<Value> {
    let provider = queries
        .resolve_provider(provider.into())
        .ok_or_else(|| error("export provider is not in the current dependency view"))?;
    if path.is_empty() {
        return Err(error("export path must contain at least one name"));
    }
    let path = path
        .into_iter()
        .map(|name| {
            CanonicalIdentifier::new(&name)
                .map_err(|cause| error(format!("invalid export path name: {cause}")))
        })
        .collect::<Result<Vec<_>>>()?;
    let namespace = match namespace {
        ExportNamespace::Type => BindingNamespace::Type,
        ExportNamespace::Value => BindingNamespace::Value,
    };
    let exports = queries
        .lookup_export(context, provider, &path, namespace)
        .map_err(one)?;
    Ok(json!({"exports": QueryJson(exports.as_slice())}))
}
