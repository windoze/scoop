use scoop_identity::{
    BindableEntity, BindingTarget, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord,
    ConeCoordinate, DeclarationScope, DefinitionOrigin, DefinitionOriginRecord,
    DefinitionOriginSubject, DefinitionOwnerChain, Effect, ExportBindingKey, GcEffect,
    NormalizedSourcePath, PackagePath, PersistentExportBindingId, PersistentFunctionId,
    SignatureTypeKey, SourceContextKey, SourceDeclarationKey, SourceDeclarationSite,
    SourceIdentity, SourceSpan,
};
use scoop_wire::WirePath;

use super::super::{ident, sp};

mod projection;

pub(crate) use projection::{
    project_dependency, project_dependency_text, project_dependency_without_default_roles,
};

pub(super) struct DependencyFunctionFixture {
    pub(super) coordinate: ConeCoordinate,
    pub(super) foundation: scoop_hir::CanonicalHirFoundation,
    pub(super) interface: scoop_hir::CrossConeHirInterfaceSectionV1,
}

impl DependencyFunctionFixture {
    pub(super) fn new(provider: &str, name: &str, result: SignatureTypeKey) -> Self {
        let coordinate = ConeCoordinate::new("test", provider, "1.0.0").unwrap();
        let origin = coordinate.identity().unwrap();
        let package = package_path(&["dependency", "api"]);
        let key = SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                origin,
                package.clone(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            Vec::new(),
        );
        let function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(key.clone()).unwrap();
        let binding: CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> =
            CborIdentityRecord::from_key(ExportBindingKey::new(
                origin,
                package,
                CanonicalIdentifier::new(name).unwrap(),
                BindingTarget::function(&key).unwrap(),
            ))
            .unwrap();
        let declaration = CallableTemplateOrigin::Function(function.id());
        let callable = scoop_hir::CallableInterfaceRecordV1::try_new(
            declaration,
            scoop_hir::PublicDeclarationOwnerV1::TopLevel,
            scoop_hir::CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
            None,
            scoop_hir::CanonicalSourceParameterShapesV1::try_new(Vec::new()).unwrap(),
            result,
            scoop_hir::CallableSourceEffectsV1::try_new(
                Effect::Ordinary,
                scoop_hir::CallableSafetyV1::Safe,
                GcEffect::Managed,
                scoop_hir::CallableImplementationV1::Scoop,
                scoop_hir::CallableOperatorRoleV1::None,
                scoop_hir::CallableInfixV1::Ordinary,
            )
            .unwrap(),
            scoop_hir::CallableModalityV1::Final,
            scoop_hir::PublicLookupAccessV1::DirectOnly,
            scoop_hir::CanonicalPersistentIdsV1::empty(),
            Vec::new(),
        )
        .unwrap();
        let source = scoop_hir::CallableSourceInterfaceV1::try_new(
            declaration,
            scoop_hir::CanonicalCallableSourceParametersV1::try_new(Vec::new()).unwrap(),
        )
        .unwrap();
        let public = scoop_hir::PublicExportBindingRecordV1::new(
            binding.id(),
            scoop_hir::ExportBindingSourceV1::DeclaredCurrent {
                declaration: BindableEntity::Function(function.id()),
            },
        );
        let mut foundation = scoop_hir::CanonicalHirFoundation::empty();
        let identity = SourceIdentity::new(
            origin,
            NormalizedSourcePath::new("declaration.scoop").unwrap(),
        )
        .unwrap();
        let context = SourceContextKey::File {
            source: identity.clone(),
        };
        let end = name.len() as u64;
        foundation
            .set_sources(vec![
                scoop_hir::SourceRecord::from_utf8(identity.clone(), name, [0, end]).unwrap(),
            ])
            .unwrap();
        foundation
            .set_source_contexts(vec![CborIdentityRecord::from_key(context.clone()).unwrap()])
            .unwrap();
        foundation
            .set_definition_origins(vec![DefinitionOriginRecord::new(
                DefinitionOriginSubject::Function(function.id()),
                DefinitionOrigin::new(identity, SourceSpan::new(0, end).unwrap(), &context)
                    .unwrap(),
            )])
            .unwrap();
        foundation.set_functions(vec![function]).unwrap();
        foundation.set_export_bindings(vec![binding]).unwrap();
        let interface = scoop_hir::CrossConeHirInterfaceSectionV1::new(
            scoop_hir::CanonicalPublicExportBindingsV1::try_new(vec![public]).unwrap(),
            scoop_hir::CanonicalNominalInterfacesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalCallableInterfacesV1::try_new(vec![callable]).unwrap(),
            scoop_hir::CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalCallableSourceInterfacesV1::try_new(vec![source]).unwrap(),
            scoop_hir::CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
            scoop_hir::CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
            Default::default(),
            Default::default(),
            Default::default(),
        );
        Self {
            coordinate,
            foundation,
            interface,
        }
    }
}

pub(crate) fn exact_import(segments: &[&str]) -> scoop_ast::ImportSyntax {
    let first = ident(segments[0]);
    let rest = segments[1..]
        .iter()
        .map(|segment| scoop_ast::QualifiedNameTailSyntax {
            dot_span: sp(),
            identifier: ident(segment),
        })
        .collect();
    scoop_ast::ImportSyntax::Exact {
        exposure: scoop_ast::ImportExposureSyntax::Local,
        selector: scoop_ast::QualifiedNameSyntax {
            first,
            rest,
            span: sp(),
        },
        alias: None,
        import_keyword_span: sp(),
        span: sp(),
    }
}

fn package_path(segments: &[&str]) -> PackagePath {
    PackagePath::from_segments(
        segments
            .iter()
            .map(|segment| CanonicalIdentifier::new(segment).unwrap())
            .collect(),
    )
}

pub(crate) fn empty_alias_expansions() -> scoop_hir::CanonicalTypeAliasExpansionsV1 {
    alias_expansions(&scoop_hir::CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap())
}

pub(crate) fn alias_expansions(
    aliases: &scoop_hir::CanonicalTypeAliasInterfacesV1,
) -> scoop_hir::CanonicalTypeAliasExpansionsV1 {
    aliases
        .expand_alias_closure(&[], &WirePath::root())
        .unwrap()
}
