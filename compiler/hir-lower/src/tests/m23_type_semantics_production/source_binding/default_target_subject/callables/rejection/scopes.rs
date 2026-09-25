use super::*;
use scoop_identity::{
    CborIdentityRecord, DeclarationName, DeclarationScope, DecodedCborIdentityRecord,
    DecodedSourceDeclarationKey, DefinitionOwnerChain, DuplicateSignatureKey, IdentityLayer,
    PendingIdentityValidation, PersistentFunctionId, SourceDeclarationKey, SourceDeclarationSite,
};

#[test]
fn default_callable_scope_checks_use_actual_keys_from_the_same_identity_graph() {
    with_hir_source(SOURCE, |output, _| {
        let fixture = Fixture::from_output(output);
        let Callable::LocalFunction {
            declaration: CallableTemplateOrigin::Function(local),
        } = target(output, "local", 0)
        else {
            panic!("local source function");
        };
        let key = fixture
            .identities
            .canonical_key::<PersistentFunctionId, SourceDeclarationKey>(local)
            .unwrap();
        let DeclarationName::Named(name) = key.name() else {
            panic!("function name");
        };
        let DuplicateSignatureKey::Function {
            type_parameter_count,
            receiver,
            parameters,
        } = key.duplicate_signature()
        else {
            panic!("function signature");
        };
        let receiver = match receiver {
            OptionalSignatureType::Present(ty) => Some(ty.as_ref().clone()),
            OptionalSignatureType::Absent => None,
        };
        let foreign = scoop_identity::CoreBuiltinNominal::Any
            .identity_record()
            .key()
            .origin();
        for (origin, owners, scope) in [
            (
                key.origin(),
                key.owners().clone(),
                DeclarationScope::ConeWide,
            ),
            (
                key.origin(),
                DefinitionOwnerChain::top_level(),
                key.scope().clone(),
            ),
            (
                foreign,
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            ),
        ] {
            let changed = SourceDeclarationKey::function(
                SourceDeclarationSite::new(origin, key.package().clone(), owners, scope).unwrap(),
                name.clone(),
                *type_parameter_count,
                receiver.clone(),
                parameters.clone(),
            );
            let record = CborIdentityRecord::<PersistentFunctionId, _>::from_key(changed).unwrap();
            let decoded: DecodedCborIdentityRecord<
                PersistentFunctionId,
                DecodedSourceDeclarationKey,
            > = decode_canonical(&encode(&record).unwrap()).unwrap();
            let mut validation = PendingIdentityValidation::new();
            validation
                .register_external_graph_authorities(&fixture.identities)
                .unwrap();
            validation.register(IdentityLayer::Hir, &decoded).unwrap();
            validation.resolve(&decoded).unwrap();
            let identities = validation.finish().unwrap();
            let mut canonical = fixture.foundation.as_canonical().clone();
            canonical.set_functions(vec![record.clone()]).unwrap();
            let artifact = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let target = direct(Declaration::Function(record.id()));
            let wrong_graph = fixture
                .source
                .bind_to_foundation(&artifact, &fixture.identities)
                .unwrap();
            assert!(matches!(
                wrong_graph.default_callable_access_subject(&target),
                Err(Error::Foundation(
                    hir::TypeFoundationBindingError::Identity(_)
                ))
            ));
            let foundation = fixture
                .source
                .bind_to_foundation(&artifact, &identities)
                .unwrap();
            let error = foundation
                .default_callable_access_subject(&target)
                .unwrap_err();
            if origin == key.origin() {
                assert!(
                    matches!(error, Error::DeclarationScope(Subject::Function(id)) if id == record.id())
                );
            } else {
                assert!(
                    matches!(error, Error::ForeignDeclaration(Subject::Function(id)) if id == record.id())
                );
            }
        }
    });
}
