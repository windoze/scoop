use super::*;

pub(super) struct Fixture {
    foundation: CanonicalHirFoundation,
    nominal: NominalInterfaceRecordV1,
    origins: Vec<DefinitionOriginRecord>,
    other_origin: DefinitionOrigin,
    pub properties: CanonicalPropertyInterfacesV1,
    pub callables: CanonicalCallableInterfacesV1,
    pub hidden: PropertyOwner,
    pub hidden_getter: PersistentPropertyAccessorId,
    pub setter: PersistentPropertyAccessorId,
}

impl Fixture {
    pub fn new() -> Self {
        let site = SourceDeclarationSite::new(
            cone().identity(),
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let nominal = CborIdentityRecord::<PersistentTypeId, _>::from_key(
            SourceDeclarationKey::nominal(site, name("Container"), SourceNominalKind::Class, 0),
        )
        .unwrap();
        let site = SourceDeclarationSite::new(
            cone().identity(),
            PackagePath::root(),
            DefinitionOwnerChain::from_outer_to_inner(vec![DefinitionOwnerAtom::Type(
                nominal.id(),
            )]),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let hidden = CborIdentityRecord::<PersistentPropertyId, _>::from_key(
            SourceDeclarationKey::property(site.clone(), name("hidden")),
        )
        .unwrap();
        let exposed = CborIdentityRecord::<PersistentPropertyId, _>::from_key(
            SourceDeclarationKey::property(site, name("exposed")),
        )
        .unwrap();
        let hidden_getter = accessor_key(hidden.id(), AccessorRole::Getter);
        let getter = accessor_key(exposed.id(), AccessorRole::Getter);
        let setter = accessor_key(exposed.id(), AccessorRole::Setter);
        let (primary_source, context, origin) = source("src/Container.scoop");
        let (other_source, other_context, other_origin) = source("src/Other.scoop");
        let origins = [
            DefinitionOriginSubject::Type(nominal.id()),
            DefinitionOriginSubject::Property(hidden.id()),
            DefinitionOriginSubject::Property(exposed.id()),
            DefinitionOriginSubject::PropertyAccessor(hidden_getter.id()),
            DefinitionOriginSubject::PropertyAccessor(getter.id()),
            DefinitionOriginSubject::PropertyAccessor(setter.id()),
        ]
        .map(|subject| DefinitionOriginRecord::new(subject, origin.clone()))
        .to_vec();
        let mut foundation = base_hir_foundation();
        foundation
            .set_sources(vec![primary_source, other_source])
            .unwrap();
        foundation
            .set_source_contexts(vec![context, other_context])
            .unwrap();
        foundation.set_definition_origins(origins.clone()).unwrap();
        foundation
            .set_types(vec![
                CoreBuiltinNominal::Unit.identity_record(),
                nominal.clone(),
            ])
            .unwrap();
        foundation
            .set_properties(vec![hidden.clone(), exposed.clone()])
            .unwrap();
        foundation
            .set_property_accessors(vec![hidden_getter.clone(), getter.clone(), setter.clone()])
            .unwrap();
        let owner = SourceNominalId::Concrete(nominal.id());
        let value_type = SignatureTypeKey::Nominal(nominal.id());
        let property = |id, accessors, visibility| {
            PropertyDeclarationRecordV1::try_new(
                id,
                PublicDeclarationOwnerV1::Nominal(owner),
                binders(),
                None,
                value_type.clone(),
                accessors,
                PropertyRepresentationV1::RuntimeAccessor,
                visibility,
            )
            .unwrap()
        };
        let hidden_property = property(
            PropertyOwner::Property(hidden.id()),
            PropertyAccessorsV1::read_only(scoop_hir::PropertyAccessorSourceV1::new(
                hidden_getter.id(),
                scoop_hir::PropertyAccessorImplementationV1::Body,
            )),
            DeclaredVisibilityV1::Private,
        );
        let exposed_property = PropertyInterfaceRecordV1::from_declaration(
            property(
                PropertyOwner::Property(exposed.id()),
                PropertyAccessorsV1::try_read_write(
                    scoop_hir::PropertyAccessorSourceV1::new(
                        getter.id(),
                        scoop_hir::PropertyAccessorImplementationV1::Body,
                    ),
                    scoop_hir::PropertyAccessorSourceV1::new(
                        setter.id(),
                        scoop_hir::PropertyAccessorImplementationV1::Body,
                    ),
                )
                .unwrap(),
                DeclaredVisibilityV1::Public,
            ),
            PropertyPublicAccessV1::DirectOnly,
            PropertySetterPublicAccessV1::Restricted,
        )
        .unwrap();
        let properties = CanonicalPropertyInterfacesV1::with_support(
            vec![exposed_property],
            vec![hidden_property],
        )
        .unwrap();
        let accessor = |id, role, visibility| {
            let (parameters, result) = match role {
                AccessorRole::Getter => (vec![], value_type.clone()),
                AccessorRole::Setter => (
                    vec![SourceParameterShapeV1::new(
                        name("next"),
                        value_type.clone(),
                    )],
                    SignatureTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
                ),
            };
            CallableDeclarationRecordV1::try_new(
                CallableTemplateOrigin::Accessor(id),
                PublicDeclarationOwnerV1::Nominal(owner),
                binders(),
                None,
                CanonicalSourceParameterShapesV1::try_new(parameters).unwrap(),
                result,
                scoop_effects(),
                CallableModalityV1::Final,
                visibility,
                CanonicalPersistentIdsV1::empty(),
                Vec::new(),
            )
            .unwrap()
        };
        let callables = CanonicalCallableInterfacesV1::with_support(
            vec![
                CallableInterfaceRecordV1::from_declaration(
                    accessor(
                        getter.id(),
                        AccessorRole::Getter,
                        DeclaredVisibilityV1::Public,
                    ),
                    PublicLookupAccessV1::DirectOnly,
                )
                .unwrap(),
            ],
            vec![
                accessor(
                    hidden_getter.id(),
                    AccessorRole::Getter,
                    DeclaredVisibilityV1::Private,
                ),
                accessor(
                    setter.id(),
                    AccessorRole::Setter,
                    DeclaredVisibilityV1::Private,
                ),
            ],
        )
        .unwrap();
        let record = NominalInterfaceRecordV1::try_new(
            owner,
            PublicNominalKindV1::Class,
            binders(),
            CanonicalSignatureTypesV1::try_new(vec![]).unwrap(),
            CanonicalPersistentIdsV1::empty(),
            CanonicalPublicMemberRefsV1::try_new(vec![PublicMemberRefV1::Property(
                PropertyOwner::Property(exposed.id()),
            )])
            .unwrap(),
            CanonicalPersistentIdsV1::empty(),
            NominalSourceShapeV1::Class(Default::default()),
            NominalDeclarationDetailsV1::new(
                NominalInheritanceModalityV1::Final,
                DeclaredVisibilityV1::Public,
                CanonicalPersistentIdsV1::empty(),
                CanonicalNestedMemberRefsV1::try_new(vec![
                    NestedSourceMemberRefV1::Property(hidden.id()),
                    NestedSourceMemberRefV1::Property(exposed.id()),
                ])
                .unwrap(),
                CanonicalNestedNominalRefsV1::default(),
                scoop_hir::NominalDispatchOrderV1::empty(PublicNominalKindV1::Class),
                scoop_hir::CanonicalNominalDispatchSelectionsV1::empty(),
                None,
                scoop_hir::NominalInstantiationConditionsV1::empty(),
                Default::default(),
                None,
            ),
        )
        .unwrap();
        Self {
            foundation,
            nominal: record,
            origins,
            other_origin,
            properties,
            callables,
            hidden: PropertyOwner::Property(hidden.id()),
            hidden_getter: hidden_getter.id(),
            setter: setter.id(),
        }
    }

    pub fn artifact(&self) -> Vec<u8> {
        let mut section = CrossConeHirInterfaceSectionV1::new(
            CanonicalPublicExportBindingsV1::try_new(vec![]).unwrap(),
            CanonicalNominalInterfacesV1::try_new(vec![self.nominal.clone()]).unwrap(),
            self.callables.clone(),
            self.properties.clone(),
            CanonicalTypeAliasInterfacesV1::try_new(vec![]).unwrap(),
            CanonicalCallableSourceInterfacesV1::try_new(vec![]).unwrap(),
            CanonicalExportDefaultTemplatesV1::try_new(vec![]).unwrap(),
            CanonicalExportConstValuesV1::try_new(vec![]).unwrap(),
            CanonicalExportDefinitionSourcesV1::try_new(vec![]).unwrap(),
            CanonicalExternalHirReferencesV1::try_new(vec![]).unwrap(),
            Default::default(),
            Default::default(),
            Default::default(),
        );
        cross_cone_artifact_for_with_hir_foundation(
            cone(),
            vec![],
            &self.foundation,
            encode(&section.index_for_wire().unwrap()).unwrap(),
        )
    }

    pub fn move_origin(&mut self, subject: DefinitionOriginSubject) {
        let record = self
            .origins
            .iter_mut()
            .find(|record| record.subject() == subject)
            .unwrap();
        *record = DefinitionOriginRecord::new(subject, self.other_origin.clone());
        self.foundation
            .set_definition_origins(self.origins.clone())
            .unwrap();
    }

    pub fn change_hidden(&mut self, owner: PublicDeclarationOwnerV1, value_type: SignatureTypeKey) {
        let record = self.properties.declaration(self.hidden).unwrap();
        let replacement = PropertyDeclarationRecordV1::try_new(
            record.declaration(),
            owner,
            record.type_parameters().clone(),
            record.receiver().cloned(),
            value_type,
            record.accessors(),
            record.representation(),
            record.declared_visibility(),
        )
        .unwrap();
        self.properties = CanonicalPropertyInterfacesV1::with_support(
            self.properties.records().to_vec(),
            vec![replacement],
        )
        .unwrap();
    }
}

fn name(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
fn binders() -> CanonicalBinderListV1 {
    CanonicalBinderListV1::try_new(vec![]).unwrap()
}
fn accessor_key(
    property: PersistentPropertyId,
    role: AccessorRole,
) -> CborIdentityRecord<PersistentPropertyAccessorId, PropertyAccessorKey> {
    CborIdentityRecord::from_key(PropertyAccessorKey::new(
        PropertyOwner::Property(property),
        role,
    ))
    .unwrap()
}

fn source(
    path: &str,
) -> (
    scoop_hir::SourceRecord,
    CborIdentityRecord<PersistentSourceContextId, SourceContextKey>,
    DefinitionOrigin,
) {
    let source =
        SourceIdentity::new(cone().identity(), NormalizedSourcePath::new(path).unwrap()).unwrap();
    let key = SourceContextKey::File {
        source: source.clone(),
    };
    let context = CborIdentityRecord::from_key(key.clone()).unwrap();
    let origin =
        DefinitionOrigin::new(source.clone(), SourceSpan::new(0, 5).unwrap(), &key).unwrap();
    (
        scoop_hir::SourceRecord::from_utf8(source, "class Container", [0, 5]).unwrap(),
        context,
        origin,
    )
}
