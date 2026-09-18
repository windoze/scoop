use scoop_identity::{
    BindingTarget, CanonicalIdentifier, CborIdentityRecord, ConeCoordinate, DeclarationScope,
    DefinitionOwnerChain, ExportBindingKey, PackagePath, SourceDeclarationKey,
    SourceDeclarationSite,
};

use super::*;

#[test]
fn non_overloadable_reexport_conflicts_with_current_declaration() {
    let current = cone("test:current:1.0.0");
    let first = nominal_candidate(current, cone("test:first:1.0.0"), "Shared", None);
    let second = nominal_candidate(
        current,
        cone("test:second:1.0.0"),
        "Shared",
        Some(location()),
    );

    let error = seal(vec![first, second]).unwrap_err();

    assert!(matches!(
        error,
        PersistentExportBindingIdentityError::DestinationConflict {
            file: 7,
            span: scoop_ast::Span { start: 11, end: 19 }
        }
    ));
}

#[test]
fn same_signature_reexport_conflicts_with_current_overload() {
    let current = cone("test:current:1.0.0");
    let first = function_candidate(current, cone("test:first:1.0.0"), "invoke", 0, None);
    let second = function_candidate(
        current,
        cone("test:second:1.0.0"),
        "invoke",
        0,
        Some(location()),
    );

    assert!(matches!(
        seal(vec![first, second]),
        Err(PersistentExportBindingIdentityError::DestinationConflict { .. })
    ));
}

#[test]
fn distinct_overloads_commit_identities_and_source_records_together() {
    let current = cone("test:current:1.0.0");
    let first = function_candidate(current, cone("test:first:1.0.0"), "invoke", 0, None);
    let second = function_candidate(
        current,
        cone("test:second:1.0.0"),
        "invoke",
        1,
        Some(location()),
    );

    let bindings = seal(vec![first, second]).unwrap();

    assert_eq!(bindings.identities.len(), 2);
    assert_eq!(bindings.surface.records().len(), 2);
    bindings
        .identities
        .validate_public_bindings(&bindings.surface)
        .unwrap();
    assert_eq!(
        bindings
            .surface
            .records()
            .iter()
            .filter(|record| matches!(record.source(), hir::ExportBindingSourceV1::Reexport { .. }))
            .count(),
        1
    );
}

fn nominal_candidate(
    exporter: scoop_identity::ConeIdentity,
    origin: scoop_identity::ConeIdentity,
    name: &str,
    location: Option<SourceLocation>,
) -> Candidate {
    let declaration = SourceDeclarationKey::nominal(
        site(origin),
        CanonicalIdentifier::new(name).unwrap(),
        scoop_identity::SourceNominalKind::Class,
        0,
    );
    candidate(
        exporter,
        name,
        BindingTarget::type_name(&declaration).unwrap(),
        hir::ImportedBindingConflictKey::Type,
        location,
    )
}

fn function_candidate(
    exporter: scoop_identity::ConeIdentity,
    origin: scoop_identity::ConeIdentity,
    name: &str,
    type_parameter_count: u32,
    location: Option<SourceLocation>,
) -> Candidate {
    let declaration = SourceDeclarationKey::function(
        site(origin),
        CanonicalIdentifier::new(name).unwrap(),
        type_parameter_count,
        None,
        Vec::new(),
    );
    candidate(
        exporter,
        name,
        BindingTarget::function(&declaration).unwrap(),
        hir::ImportedBindingConflictKey::Overload(declaration.duplicate_signature().clone()),
        location,
    )
}

fn candidate(
    exporter: scoop_identity::ConeIdentity,
    name: &str,
    target: BindingTarget,
    conflict: hir::ImportedBindingConflictKey,
    location: Option<SourceLocation>,
) -> Candidate {
    let identity = CborIdentityRecord::from_key(ExportBindingKey::new(
        exporter,
        PackagePath::root(),
        CanonicalIdentifier::new(name).unwrap(),
        target,
    ))
    .unwrap();
    let source = if location.is_some() {
        hir::ExportBindingSourceV1::Reexport { routes: routes() }
    } else {
        hir::ExportBindingSourceV1::DeclaredCurrent {
            declaration: target.target(),
        }
    };
    Candidate {
        identity,
        conflict,
        source,
        location,
    }
}

fn routes() -> hir::CanonicalReexportRoutesV1 {
    let provider = cone("test:route-provider:1.0.0");
    let declaration = SourceDeclarationKey::nominal(
        site(provider),
        CanonicalIdentifier::new("RouteType").unwrap(),
        scoop_identity::SourceNominalKind::Class,
        0,
    );
    let binding = CborIdentityRecord::from_key(ExportBindingKey::new(
        provider,
        PackagePath::root(),
        CanonicalIdentifier::new("origin").unwrap(),
        BindingTarget::type_name(&declaration).unwrap(),
    ))
    .unwrap();
    let route = hir::ReexportRouteV1::try_new(
        provider,
        vec![hir::ReexportRouteHopV1::new(provider, binding.id())],
    )
    .unwrap();
    hir::CanonicalReexportRoutesV1::try_new(vec![route]).unwrap()
}

fn site(origin: scoop_identity::ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        origin,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

fn location() -> SourceLocation {
    SourceLocation {
        file: 7,
        span: scoop_ast::Span { start: 11, end: 19 },
    }
}

fn cone(coordinate: &str) -> scoop_identity::ConeIdentity {
    let mut parts = coordinate.split(':');
    ConeCoordinate::new(
        parts.next().unwrap(),
        parts.next().unwrap(),
        parts.next().unwrap(),
    )
    .unwrap()
    .identity()
    .unwrap()
}
