use super::*;

pub(super) fn functions(
    functions: &Arena<hir::Function>,
    identities: &hir::HirFunctionIdentities,
) -> hir::HirExportDefinitionOrigins {
    let source = scoop_identity::SourceIdentity::single_file();
    let context = scoop_identity::SourceContextKey::File {
        source: source.clone(),
    };
    let records = functions
        .iter()
        .filter_map(|(id, function)| {
            let subject = match identities[id].source_identity()? {
                hir::HirSourceFunctionIdentity::Plain(record) => {
                    scoop_identity::DefinitionOriginSubject::Function(record.id())
                }
                hir::HirSourceFunctionIdentity::Generic(record) => {
                    scoop_identity::DefinitionOriginSubject::GenericFunction(record.id())
                }
            };
            let span = function.span;
            let origin = scoop_identity::DefinitionOrigin::new(
                source.clone(),
                scoop_identity::SourceSpan::new(u64::from(span.start), u64::from(span.end))
                    .unwrap(),
                &context,
            )
            .unwrap();
            Some(scoop_identity::DefinitionOriginRecord::new(subject, origin))
        })
        .collect();
    hir::HirExportDefinitionOrigins::canonicalize(records).unwrap()
}
