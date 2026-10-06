use super::*;
use crate::*;
use scoop_identity::{
    CallableTemplateOrigin, DefinitionOwnerAtom, DispatchSlotKey, PersistentDispatchSlotId,
    PersistentFunctionId, SignatureTypeKey,
};

#[test]
fn inherited_defaults_close_foreign_targets_without_source_lookup_witnesses() {
    let current = cone("consumer");
    let provider = cone("dispatch-provider");
    let interface = PersistentTypeId::from_source_declaration(&SourceDeclarationKey::nominal(
        site(provider),
        CanonicalIdentifier::new("Interface").unwrap(),
        SourceNominalKind::Interface,
        0,
    ))
    .unwrap();
    let function = PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(interface)]),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new("value").unwrap(),
        0,
        None,
        Vec::new(),
    ))
    .unwrap();
    let slot =
        PersistentDispatchSlotId::from_key(&DispatchSlotKey::interface_method(function)).unwrap();
    let target = ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(function));
    let parent = ExternalHirTargetV1::Nominal(NominalDeclarationOwner::Concrete(interface));
    let mut authority = Authority::new(current)
        .with_origin(target, provider)
        .with_origin(parent, provider);
    let mut parts = EmptyParts::new(CanonicalPublicExportBindingsV1::try_new(Vec::new()).unwrap());

    let choices =
        CanonicalNominalDispatchSelectionsV1::try_new(vec![NominalDispatchSelectionV1::new(
            NominalDispatchSelectionRoleV1::Interface {
                interface: SignatureTypeKey::Nominal(interface),
            },
            SignatureTypeKey::Nominal(interface),
            slot,
            InheritanceSourceSlotSelectionV1::InterfaceDefault(
                InheritanceCallableDeclarationV1::Function(function),
            ),
        )])
        .unwrap();
    parts.nominal_interfaces = CanonicalNominalInterfacesV1::try_new(
        ["First", "Second"]
            .map(|name| {
                NominalInterfaceRecordV1::try_new(
                    nominal(current, name),
                    PublicNominalKindV1::Class,
                    CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
                    CanonicalSignatureTypesV1::try_new(vec![SignatureTypeKey::Nominal(interface)])
                        .unwrap(),
                    CanonicalPersistentIdsV1::empty(),
                    CanonicalPublicMemberRefsV1::default(),
                    CanonicalPersistentIdsV1::empty(),
                    NominalSourceShapeV1::Class(Default::default()),
                    NominalDeclarationDetailsV1::new(
                        NominalInheritanceModalityV1::Final,
                        DeclaredVisibilityV1::Public,
                        CanonicalPersistentIdsV1::empty(),
                        CanonicalNestedMemberRefsV1::try_new(Vec::new()).unwrap(),
                        CanonicalNestedNominalRefsV1::default(),
                        NominalDispatchOrderV1::empty(PublicNominalKindV1::Class),
                        choices.clone(),
                        None,
                        crate::NominalInstantiationConditionsV1::empty(),
                        Default::default(),
                        None,
                        None,
                    ),
                )
                .unwrap()
            })
            .to_vec(),
    )
    .unwrap();
    let references =
        CanonicalExternalHirReferencesV1::from_interface_parts(parts.input(), &[], &mut authority)
            .unwrap();
    assert_eq!(references.records().len(), 2);
    let inherited = references.get(target).unwrap();
    assert_eq!(inherited.origin(), provider);
    assert_eq!(
        inherited.roles().roles(),
        &[ExternalHirReferenceRoleV1::InheritanceDependency]
    );
    assert!(inherited.witnesses().is_empty());
    let section = |references| {
        CrossConeHirInterfaceSectionV1::new(
            parts.public_bindings.clone(),
            parts.nominal_interfaces.clone(),
            parts.callable_interfaces.clone(),
            parts.property_interfaces.clone(),
            parts.type_aliases.clone(),
            parts.source_interfaces.clone(),
            parts.default_templates.clone(),
            parts.constants.clone(),
            CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
            references,
            Default::default(),
            Default::default(),
            Default::default(),
        )
    };
    section(references.clone())
        .validate_external_reference_closure(&mut authority, &WirePath::root())
        .unwrap();
    let changed = |origin, role| {
        let record = ExternalHirReferenceV1::try_new(
            origin,
            target,
            CanonicalExternalHirReferenceRolesV1::try_new(vec![role]).unwrap(),
            CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
            Default::default(),
            Default::default(),
        )
        .unwrap();
        CanonicalExternalHirReferencesV1::try_new(vec![
            references.get(parent).unwrap().clone(),
            record,
        ])
        .unwrap()
    };
    let mut verify =
        |references| section(references).validate_inheritance_reference_closure(&mut authority);
    use ExternalHirInheritanceClosureValidationError as Error;
    let missing =
        CanonicalExternalHirReferencesV1::try_new(vec![references.get(parent).unwrap().clone()])
            .unwrap();
    assert!(matches!(verify(missing), Err(Error::MissingReference(actual)) if actual == target));
    assert!(
        matches!(verify(changed(provider, ExternalHirReferenceRoleV1::SignatureDependency)), Err(Error::MissingRole(actual)) if actual == target)
    );
    assert!(
        matches!(verify(changed(cone("wrong-provider"), ExternalHirReferenceRoleV1::InheritanceDependency)), Err(Error::Origin { expected, .. }) if expected == provider)
    );
    let excess = ExternalHirReferenceV1::try_new(
        provider,
        parent,
        CanonicalExternalHirReferenceRolesV1::try_new(vec![
            ExternalHirReferenceRoleV1::SignatureDependency,
            ExternalHirReferenceRoleV1::InheritanceDependency,
        ])
        .unwrap(),
        CanonicalDependencyBindingWitnessesV1::try_new(Vec::new()).unwrap(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert!(
        matches!(verify(CanonicalExternalHirReferencesV1::try_new(vec![inherited.clone(), excess]).unwrap()), Err(Error::ExtraRole(actual)) if actual == parent)
    );
}
