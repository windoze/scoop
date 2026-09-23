use scoop_hir::{ExportDefaultTemplateV1, OdrFreeHirFoundation};
use scoop_identity::CallableOwner;

use super::*;

pub(super) fn replace_root(
    fixture: &mut CallableSourceSurface,
    root: PersistentLexicalRootV1,
    origin: ExportDefinitionSourceV1,
) {
    let interface = &fixture.interface;
    let template = &interface.default_templates().records()[0];
    let replacement = ExportDefaultTemplateV1::try_new(
        template.key(),
        root,
        template.definition_path().clone(),
        template.locals().clone(),
        template.body().clone(),
        template.result().clone(),
        template.allows_suspend(),
        template.type_parameters().clone(),
        template.receiver().clone(),
        template.value_parameters().clone(),
        template.references().clone(),
        origin.clone(),
    )
    .unwrap();
    let mut origins = interface.definition_sources().sources().to_vec();
    origins.push(origin);
    origins.sort();
    origins.dedup();
    fixture.interface = CrossConeHirInterfaceSectionV1::new(
        interface.public_bindings().clone(),
        interface.nominal_interfaces().clone(),
        interface.callable_interfaces().clone(),
        interface.property_interfaces().clone(),
        interface.type_aliases().clone(),
        interface.source_interfaces().clone(),
        CanonicalExportDefaultTemplatesV1::try_new(vec![replacement]).unwrap(),
        interface.constants().clone(),
        CanonicalExportDefinitionSourcesV1::try_new(origins).unwrap(),
        interface.external_references().clone(),
    );
}

pub(super) fn add_sibling(
    fixture: &mut CallableSourceSurface,
    another_file: bool,
) -> (PersistentLexicalRootV1, ExportDefinitionSourceV1) {
    let original = OdrFreeHirFoundation::try_new(fixture.foundation.clone()).unwrap();
    let CallableTemplateOrigin::Function(current) = fixture.owner else {
        panic!("fixture function")
    };
    let declaration = original
        .definition_origin(DefinitionOriginSubject::Function(current))
        .unwrap();
    let source = if another_file {
        SourceIdentity::new(
            fixture.cone.identity(),
            NormalizedSourcePath::new("src/Other.scoop").unwrap(),
        )
        .unwrap()
    } else {
        declaration.origin().source().clone()
    };
    let parameters = fixture
        .interface
        .source_interfaces()
        .get(fixture.owner)
        .unwrap();
    let function = |name| {
        CborIdentityRecord::<PersistentFunctionId, _>::from_key(SourceDeclarationKey::function(
            SourceDeclarationSite::new(
                fixture.cone.identity(),
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            parameters
                .parameters()
                .parameters()
                .iter()
                .map(|parameter| parameter.value_type().clone())
                .collect(),
        ))
        .unwrap()
    };
    let current_record = function("consume");
    assert_eq!(current_record.id(), current);
    let other = function("other");
    let root = PersistentLexicalRootV1::Function(other.id());
    let file_context = SourceContextKey::File {
        source: source.clone(),
    };
    let body_context = SourceContextKey::Callable {
        source: source.clone(),
        owner: CallableOwner::Function(other.id()),
    };
    let mut contexts: Vec<_> = original
        .source_context_records()
        .map(|(_, key)| {
            CborIdentityRecord::<PersistentSourceContextId, _>::from_key(key.clone()).unwrap()
        })
        .collect();
    if another_file {
        contexts.push(CborIdentityRecord::from_key(file_context.clone()).unwrap());
        let mut sources = original.source_records().to_vec();
        sources.push(scoop_hir::SourceRecord::from_utf8(source.clone(), "other", [0, 5]).unwrap());
        fixture.foundation.set_sources(sources).unwrap();
    }
    contexts.push(CborIdentityRecord::from_key(body_context.clone()).unwrap());
    fixture.foundation.set_source_contexts(contexts).unwrap();
    let mut origins: Vec<_> = fixture
        .interface
        .nominal_interfaces()
        .records()
        .iter()
        .map(|nominal| {
            let subject = match nominal.declaration() {
                SourceNominalId::Concrete(id) => DefinitionOriginSubject::Type(id),
                SourceNominalId::GenericTemplate(id) => DefinitionOriginSubject::GenericType(id),
            };
            original.definition_origin(subject).unwrap().clone()
        })
        .collect();
    origins.push(declaration.clone());
    origins.push(DefinitionOriginRecord::new(
        DefinitionOriginSubject::Function(other.id()),
        DefinitionOrigin::new(
            source.clone(),
            SourceSpan::new(0, 5).unwrap(),
            &file_context,
        )
        .unwrap(),
    ));
    fixture.foundation.set_definition_origins(origins).unwrap();
    fixture
        .foundation
        .set_functions(vec![current_record, other])
        .unwrap();
    (
        root,
        ExportDefinitionSourceV1::new(
            DefinitionOrigin::new(source, SourceSpan::new(0, 5).unwrap(), &body_context).unwrap(),
        ),
    )
}
