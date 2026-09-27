use scoop_hir as hir;
use scoop_identity::{
    BindableEntity, BindingTarget, CallableTemplateOrigin, CanonicalIdentifier, CborIdentityRecord,
    ConeCoordinate, ConeIdentity, DeclarationScope, DefinitionOwnerAtom, DefinitionOwnerChain,
    Effect, ExportBindingKey, GcEffect, PackagePath, PersistentExportBindingId,
    PersistentFunctionId, PersistentObjectValueId, PersistentTypeId, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};

pub(super) struct ProviderFixture {
    coordinate: ConeCoordinate,
    foundation: hir::CanonicalHirFoundation,
    interface: hir::CrossConeHirInterfaceSectionV1,
}

impl ProviderFixture {
    pub(super) fn empty(coordinate: ConeCoordinate) -> Self {
        Self {
            coordinate,
            foundation: hir::CanonicalHirFoundation::empty(),
            interface: interface(Vec::new(), Vec::new(), Vec::new()),
        }
    }

    pub(super) fn with_nested_type(
        coordinate: ConeCoordinate,
        package: PackagePath,
        outer_name: &str,
        nested_name: &str,
    ) -> Self {
        let origin = coordinate.identity().unwrap();
        let outer_key = nominal_key(
            origin,
            package.clone(),
            DefinitionOwnerChain::top_level(),
            outer_name,
        );
        let outer: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(outer_key.clone()).unwrap();
        let outer_binding = type_binding(origin, package.clone(), outer_name, &outer_key);
        let nested_key = nominal_key(
            origin,
            package.clone(),
            DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(outer.id())]),
            nested_name,
        );
        let nested: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(nested_key.clone()).unwrap();
        let nested_binding = type_binding(origin, package, nested_name, &nested_key);

        let mut foundation = hir::CanonicalHirFoundation::empty();
        foundation
            .set_types(vec![outer.clone(), nested.clone()])
            .unwrap();
        foundation
            .set_export_bindings(vec![outer_binding.clone(), nested_binding.clone()])
            .unwrap();
        Self {
            coordinate,
            foundation,
            interface: interface(
                vec![
                    public_binding(outer_binding.id(), BindableEntity::Type(outer.id())),
                    public_binding(nested_binding.id(), BindableEntity::Type(nested.id())),
                ],
                vec![
                    nominal_record(outer.id(), vec![nested_binding.id()]),
                    nominal_record(nested.id(), Vec::new()),
                ],
                Vec::new(),
            ),
        }
    }

    pub(super) fn with_function(
        coordinate: ConeCoordinate,
        package: PackagePath,
        name: &str,
        parameter_count: usize,
    ) -> Self {
        let origin = coordinate.identity().unwrap();
        let support_key = nominal_key(
            origin,
            package.clone(),
            DefinitionOwnerChain::top_level(),
            "FunctionSupport",
        );
        let support: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(support_key).unwrap();
        let parameter_types = vec![SignatureTypeKey::Nominal(support.id()); parameter_count];
        let function_key = SourceDeclarationKey::function(
            declaration_site(origin, package.clone(), DefinitionOwnerChain::top_level()),
            CanonicalIdentifier::new(name).unwrap(),
            0,
            None,
            parameter_types.clone(),
        );
        let function: CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(function_key.clone()).unwrap();
        let function_binding = value_binding(
            origin,
            package,
            name,
            BindingTarget::function(&function_key).unwrap(),
        );

        let mut foundation = hir::CanonicalHirFoundation::empty();
        foundation.set_types(vec![support.clone()]).unwrap();
        foundation.set_functions(vec![function.clone()]).unwrap();
        foundation
            .set_export_bindings(vec![function_binding.clone()])
            .unwrap();
        Self {
            coordinate,
            foundation,
            interface: interface(
                vec![public_binding(
                    function_binding.id(),
                    BindableEntity::Function(function.id()),
                )],
                vec![nominal_record(support.id(), Vec::new())],
                vec![callable_record(
                    function.id(),
                    parameter_types,
                    SignatureTypeKey::Nominal(support.id()),
                )],
            ),
        }
    }

    pub(super) fn with_object(
        coordinate: ConeCoordinate,
        package: PackagePath,
        name: &str,
    ) -> Self {
        let origin = coordinate.identity().unwrap();
        let object_key = SourceDeclarationKey::nominal(
            declaration_site(origin, package.clone(), DefinitionOwnerChain::top_level()),
            CanonicalIdentifier::new(name).unwrap(),
            SourceNominalKind::Object,
            0,
        );
        let object_type: CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(object_key.clone()).unwrap();
        let object_value: CborIdentityRecord<PersistentObjectValueId, SourceDeclarationKey> =
            CborIdentityRecord::from_key(object_key.clone()).unwrap();
        let type_binding = type_binding(origin, package.clone(), name, &object_key);
        let value_binding = value_binding(
            origin,
            package,
            name,
            BindingTarget::object_value(&object_key).unwrap(),
        );

        let mut foundation = hir::CanonicalHirFoundation::empty();
        foundation.set_types(vec![object_type.clone()]).unwrap();
        foundation
            .set_object_values(vec![object_value.clone()])
            .unwrap();
        foundation
            .set_export_bindings(vec![type_binding.clone(), value_binding.clone()])
            .unwrap();
        Self {
            coordinate,
            foundation,
            interface: interface(
                vec![
                    public_binding(type_binding.id(), BindableEntity::Type(object_type.id())),
                    public_binding(
                        value_binding.id(),
                        BindableEntity::ObjectValue(object_value.id()),
                    ),
                ],
                vec![object_record(object_type.id(), object_value.id())],
                Vec::new(),
            ),
        }
    }

    pub(super) const fn coordinate(&self) -> &ConeCoordinate {
        &self.coordinate
    }

    pub(super) fn identity(&self) -> ConeIdentity {
        self.coordinate.identity().unwrap()
    }

    pub(super) const fn foundation(&self) -> &hir::CanonicalHirFoundation {
        &self.foundation
    }

    pub(super) const fn interface(&self) -> &hir::CrossConeHirInterfaceSectionV1 {
        &self.interface
    }
}

fn nominal_key(
    origin: ConeIdentity,
    package: PackagePath,
    owners: DefinitionOwnerChain,
    name: &str,
) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        declaration_site(origin, package, owners),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Class,
        0,
    )
}

fn declaration_site(
    origin: ConeIdentity,
    package: PackagePath,
    owners: DefinitionOwnerChain,
) -> SourceDeclarationSite {
    SourceDeclarationSite::new(origin, package, owners, DeclarationScope::ConeWide).unwrap()
}

fn type_binding(
    origin: ConeIdentity,
    package: PackagePath,
    name: &str,
    target: &SourceDeclarationKey,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    value_binding(
        origin,
        package,
        name,
        BindingTarget::type_name(target).unwrap(),
    )
}

fn value_binding(
    origin: ConeIdentity,
    package: PackagePath,
    name: &str,
    target: BindingTarget,
) -> CborIdentityRecord<PersistentExportBindingId, ExportBindingKey> {
    CborIdentityRecord::from_key(ExportBindingKey::new(
        origin,
        package,
        CanonicalIdentifier::new(name).unwrap(),
        target,
    ))
    .unwrap()
}

fn public_binding(
    binding: PersistentExportBindingId,
    declaration: BindableEntity,
) -> hir::PublicExportBindingRecordV1 {
    hir::PublicExportBindingRecordV1::new(
        binding,
        hir::ExportBindingSourceV1::DeclaredCurrent { declaration },
    )
}

fn nominal_record(
    declaration: PersistentTypeId,
    nested_bindings: Vec<PersistentExportBindingId>,
) -> hir::NominalInterfaceRecordV1 {
    crate::nominal_interface_fixture::public_record(
        hir::SourceNominalId::Concrete(declaration),
        hir::PublicNominalKindV1::Class,
        hir::CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalPersistentIdsV1::try_new(nested_bindings).unwrap(),
        hir::NominalSourceShapeV1::Class(Default::default()),
    )
    .unwrap()
}

fn object_record(
    declaration: PersistentTypeId,
    value: PersistentObjectValueId,
) -> hir::NominalInterfaceRecordV1 {
    crate::nominal_interface_fixture::public_record(
        hir::SourceNominalId::Concrete(declaration),
        hir::PublicNominalKindV1::Object,
        hir::CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalSignatureTypesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalPublicMemberRefsV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalPersistentIdsV1::try_new(Vec::new()).unwrap(),
        hir::NominalSourceShapeV1::Object(hir::ObjectSourceShapeV1::new(
            hir::ObjectSourceKindV1::Standalone,
            value,
            Default::default(),
        )),
    )
    .unwrap()
}

fn callable_record(
    declaration: PersistentFunctionId,
    parameter_types: Vec<SignatureTypeKey>,
    result: SignatureTypeKey,
) -> hir::CallableInterfaceRecordV1 {
    hir::CallableInterfaceRecordV1::try_new(
        CallableTemplateOrigin::Function(declaration),
        hir::PublicDeclarationOwnerV1::TopLevel,
        hir::CanonicalBinderListV1::try_new(Vec::new()).unwrap(),
        None,
        hir::CanonicalSourceParameterShapesV1::try_new(
            parameter_types
                .into_iter()
                .enumerate()
                .map(|(index, value_type)| {
                    let name = format!("p{index}");
                    hir::SourceParameterShapeV1::new(
                        CanonicalIdentifier::new(&name).unwrap(),
                        value_type,
                    )
                })
                .collect(),
        )
        .unwrap(),
        result,
        hir::CallableSourceEffectsV1::try_new(
            Effect::Ordinary,
            hir::CallableSafetyV1::Safe,
            GcEffect::Managed,
            hir::CallableImplementationV1::Scoop,
            hir::CallableOperatorRoleV1::None,
            hir::CallableInfixV1::Ordinary,
        )
        .unwrap(),
        hir::CallableModalityV1::Final,
        hir::PublicLookupAccessV1::DirectOnly,
        scoop_hir::CanonicalPersistentIdsV1::empty(),
    )
    .unwrap()
}

fn interface(
    bindings: Vec<hir::PublicExportBindingRecordV1>,
    nominals: Vec<hir::NominalInterfaceRecordV1>,
    callables: Vec<hir::CallableInterfaceRecordV1>,
) -> hir::CrossConeHirInterfaceSectionV1 {
    hir::CrossConeHirInterfaceSectionV1::new(
        hir::CanonicalPublicExportBindingsV1::try_new(bindings).unwrap(),
        hir::CanonicalNominalInterfacesV1::try_new(nominals).unwrap(),
        hir::CanonicalCallableInterfacesV1::try_new(callables).unwrap(),
        hir::CanonicalPropertyInterfacesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalTypeAliasInterfacesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalCallableSourceInterfacesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalExportDefaultTemplatesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalExportConstValuesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalExportDefinitionSourcesV1::try_new(Vec::new()).unwrap(),
        hir::CanonicalExternalHirReferencesV1::try_new(Vec::new()).unwrap(),
    )
}

pub(super) fn package_path(segments: &[&str]) -> PackagePath {
    PackagePath::from_segments(
        segments
            .iter()
            .map(|segment| CanonicalIdentifier::new(segment).unwrap())
            .collect(),
    )
}
