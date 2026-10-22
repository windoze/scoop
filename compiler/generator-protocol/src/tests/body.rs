use super::*;

mod application;
mod control_flow;
mod operations;

fn origin() -> hir::ExportDefinitionSourceV1 {
    let source = SourceIdentity::single_file();
    let context = SourceContextKey::File {
        source: source.clone(),
    };
    hir::ExportDefinitionSourceV1::new(
        DefinitionOrigin::new(source, SourceSpan::new(2, 7).unwrap(), &context).unwrap(),
    )
}

fn expression(
    kind: hir::DefaultExpressionKindV1,
    ty: SignatureTypeKey,
) -> hir::DefaultExpressionV1 {
    let origin = origin();
    hir::DefaultExpressionV1::try_new(
        kind,
        ty,
        origin.clone(),
        EvaluationOrigin::at_definition(origin.origin()),
    )
    .unwrap()
}

fn statement(kind: hir::DefaultStatementKindV1) -> hir::DefaultStatementV1 {
    hir::DefaultStatementV1::try_new(kind, origin()).unwrap()
}

fn unit() -> hir::DefaultExpressionV1 {
    expression(
        hir::DefaultExpressionKindV1::UnitLiteral,
        SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
    )
}

fn core_type(name: &str) -> SignatureTypeKey {
    SignatureTypeKey::Nominal(
        PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeIdentity::CORE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            SourceNominalKind::Struct,
            0,
        ))
        .unwrap(),
    )
}

fn binder(depth: u32, index: u32) -> SignatureTypeKey {
    SignatureTypeKey::Binder { depth, index }
}

fn path(role: StructuralDefinitionSiteRole, ordinal: u32) -> StructuralDefinitionPath {
    StructuralDefinitionPath::from_first(StructuralPathSegment::new(role, ordinal), [])
}

fn function_key(name: &str, arity: u32) -> SourceDeclarationKey {
    SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        arity,
        None,
        vec![],
    )
}
