use super::*;
use scoop_hir::{CanonicalHirFoundation, DecodedHirFoundation};

type Nominal = CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>;
type Unit = CborIdentityRecord<PersistentInitializationUnitId, InitializationUnitKey>;
type Value = CborIdentityRecord<PersistentObjectValueId, SourceDeclarationKey>;
type Accessor = CborIdentityRecord<PersistentPropertyAccessorId, PropertyAccessorKey>;

pub(in crate::cross_cone_type_bridge) struct Fixture {
    pub graph: ValidatedIdentityGraph,
    pub types: CanonicalParamFreeMirTypeExportsV1,
    pub callables: CanonicalMirCallableBindingsV1,
    pub objects: Vec<Nominal>,
    pub values: Vec<Value>,
    pub backings: Vec<crate::GeneratedNominalRecord>,
    pub units: Vec<Unit>,
    pub ensures: Vec<crate::GeneratedCallableRecord>,
    pub accessors: Vec<Accessor>,
    pub unit_exact: PersistentExactTypeId,
}
impl Fixture {
    pub fn new() -> Self {
        let objects: Vec<_> = [ConeIdentity::SINGLE_FILE, ConeIdentity::CORE]
            .into_iter()
            .map(|cone| {
                CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
                    site(cone, None),
                    name("Registry"),
                    SourceNominalKind::Object,
                    0,
                ))
                .unwrap()
            })
            .collect();
        let unit_type: Nominal = CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
            site(ConeIdentity::CORE, None),
            name("Unit"),
            SourceNominalKind::Struct,
            0,
        ))
        .unwrap();
        let values: Vec<Value> = objects
            .iter()
            .map(|object| CborIdentityRecord::from_key(object.key().clone()).unwrap())
            .collect();
        let backings: Vec<_> = objects
            .iter()
            .map(|object| {
                CborIdentityRecord::from_key(GeneratedNominalKey::ObjectBackingClass {
                    object: object.id(),
                })
                .unwrap()
            })
            .collect();
        let properties: Vec<CborIdentityRecord<scoop_identity::PersistentPropertyId, _>> =
            [None, Some(objects[1].id())]
                .into_iter()
                .map(|owner| {
                    CborIdentityRecord::from_key(SourceDeclarationKey::property(
                        site(ConeIdentity::CORE, owner),
                        name("setting"),
                    ))
                    .unwrap()
                })
                .collect();
        let extensions: Vec<CborIdentityRecord<scoop_identity::PersistentExtensionPropertyId, _>> =
            [0, 1]
                .into_iter()
                .map(|arity| {
                    CborIdentityRecord::from_key(SourceDeclarationKey::extension_property(
                        site(ConeIdentity::CORE, None),
                        name(if arity == 0 {
                            "extensionSetting"
                        } else {
                            "genericSetting"
                        }),
                        arity,
                        if arity == 0 {
                            scoop_identity::SignatureTypeKey::Nominal(unit_type.id())
                        } else {
                            scoop_identity::SignatureTypeKey::Binder { depth: 0, index: 0 }
                        },
                    ))
                    .unwrap()
                })
                .collect();
        let mut accessors: Vec<Accessor> = properties
            .iter()
            .map(|property| {
                CborIdentityRecord::from_key(PropertyAccessorKey::new(
                    PropertyOwner::Property(property.id()),
                    AccessorRole::Getter,
                ))
                .unwrap()
            })
            .collect();
        accessors.push(
            CborIdentityRecord::from_key(PropertyAccessorKey::new(
                PropertyOwner::ExtensionProperty(extensions[0].id()),
                AccessorRole::Getter,
            ))
            .unwrap(),
        );
        let units = vec![
            CborIdentityRecord::from_key(InitializationUnitKey::Object(objects[0].id())).unwrap(),
            CborIdentityRecord::from_key(InitializationUnitKey::Companion(objects[1].id()))
                .unwrap(),
            CborIdentityRecord::from_key(InitializationUnitKey::TopLevelProperty(
                properties[0].id(),
            ))
            .unwrap(),
            CborIdentityRecord::from_key(InitializationUnitKey::Object(unit_type.id())).unwrap(),
            CborIdentityRecord::from_key(InitializationUnitKey::ExtensionProperty(
                extensions[0].id(),
            ))
            .unwrap(),
            CborIdentityRecord::from_key(
                InitializationUnitKey::GenericDelegatedExtensionApplication {
                    property: extensions[1].id(),
                    receiver_arguments: scoop_identity::NonEmptyVec::new(vec![exact(
                        unit_type.id(),
                    )])
                    .unwrap(),
                },
            )
            .unwrap(),
        ];
        let ensures: Vec<_> = units[..2]
            .iter()
            .map(|unit| {
                CborIdentityRecord::from_key(GeneratedCallableKey::Initialization {
                    unit: unit.id(),
                    role: InitializationCallableRole::Ensure,
                })
                .unwrap()
            })
            .collect();
        let mut hir = CanonicalHirFoundation::empty();
        hir.set_types(objects.iter().cloned().chain([unit_type.clone()]).collect())
            .unwrap();
        hir.set_generated_types(backings.clone()).unwrap();
        hir.set_object_values(values.clone()).unwrap();
        hir.set_initialization_units(units.clone()).unwrap();
        hir.set_properties(properties).unwrap();
        hir.set_extension_properties(extensions).unwrap();
        hir.set_property_accessors(accessors.clone()).unwrap();
        hir.set_exact_types(
            objects
                .iter()
                .map(|object| object.id())
                .chain(backings.iter().map(|backing| backing.id()))
                .chain([unit_type.id()])
                .map(|nominal| {
                    CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap()
                })
                .collect(),
        )
        .unwrap();
        let unit_exact = exact(unit_type.id());
        let signature = ExactCallableSignature::new(Effect::Ordinary, None, vec![], unit_exact);
        let mut mir = crate::CanonicalMirFoundation::empty();
        mir.set_generated_callables(ensures.clone()).unwrap();
        mir.set_callable_signatures(
            ensures
                .iter()
                .map(|ensure| {
                    crate::CallableSignatureRecord::new(
                        crate::CallableSignatureSubject::strong(
                            StrongCallableDefinitionOwner::GeneratedCallable(ensure.id())
                                .callable_owner(),
                        ),
                        signature.clone(),
                    )
                })
                .collect(),
        )
        .unwrap();
        let hir: DecodedHirFoundation =
            decode_canonical(&encode(&hir).unwrap(), DecodeLimits::default()).unwrap();
        let mir: crate::DecodedMirFoundation =
            decode_canonical(&encode(&mir).unwrap(), DecodeLimits::default()).unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_authority(ConeIdentity::SINGLE_FILE)
            .unwrap();
        pending.register_authority(ConeIdentity::CORE).unwrap();
        hir.register_identities(&mut pending).unwrap();
        mir.register_identities(&mut pending).unwrap();
        hir.resolve_identities(&mut pending).unwrap();
        mir.resolve_identities(&mut pending).unwrap();
        let mut graph = pending.finish().unwrap();
        let foundation = crate::OdrFreeMirFoundation::from_validated(
            mir.validate(&mut graph, &mut meter()).unwrap(),
        )
        .unwrap();
        let authority = MirTypeBridgeAuthority {
            identities: &graph,
            foundation: &foundation,
        };
        let mut records = vec![
            ParamFreeMirTypeExportV1::try_new(
                authority,
                unit_exact,
                MirTypeOriginV1::SourceNominal(unit_type.id()),
                MirTypeFactsV1::try_new(MirValueKindV1::ZeroSizedValue, MirGcKindV1::GcFree)
                    .unwrap(),
                MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Unit),
                no_bases(),
            )
            .unwrap(),
        ];
        for (object, backing) in objects.iter().zip(&backings) {
            records.push(
                ParamFreeMirTypeExportV1::try_new(
                    authority,
                    exact(object.id()),
                    MirTypeOriginV1::SourceNominal(object.id()),
                    reference_facts(),
                    MirTypeRepresentationV1::Object {
                        backing: exact(backing.id()),
                    },
                    no_bases(),
                )
                .unwrap(),
            );
            records.push(
                ParamFreeMirTypeExportV1::try_new(
                    authority,
                    exact(backing.id()),
                    MirTypeOriginV1::GeneratedNominal {
                        nominal: backing.id(),
                        role: backing.key().clone(),
                    },
                    reference_facts(),
                    MirTypeRepresentationV1::ObjectBacking {
                        declared_fields: vec![],
                    },
                    no_bases(),
                )
                .unwrap(),
            );
        }
        let types = CanonicalParamFreeMirTypeExportsV1::try_new(records).unwrap();
        let authority = MirCallableBridgeAuthority {
            identities: &graph,
            foundation: &foundation,
            types: &types,
        };
        let signature = MirBridgeCallableSignatureV1::new(signature, crate::GcEffect::Managed);
        let callables = CanonicalMirCallableBindingsV1::try_new(
            ensures
                .iter()
                .zip(&units)
                .map(|(ensure, unit)| {
                    let origin = MirCallableOriginV1::Generated {
                        callable: ensure.id(),
                        role: ensure.key().clone(),
                    };
                    ParamFreeMirCallableBindingV1::try_new(
                        authority,
                        origin.clone(),
                        origin.implementation(),
                        signature.clone(),
                        signature.clone(),
                        MirCallableLoweringRoleV1::ObjectEnsure { unit: unit.id() },
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap();
        Self {
            graph,
            types,
            callables,
            objects,
            values,
            backings,
            units,
            ensures,
            accessors,
            unit_exact,
        }
    }
    pub fn authority(&self) -> MirObjectBridgeAuthority<'_> {
        MirObjectBridgeAuthority {
            identities: &self.graph,
            types: &self.types,
            callables: &self.callables,
        }
    }
    pub fn object(&self, index: usize) -> ParamFreeMirObjectValueV1 {
        ParamFreeMirObjectValueV1::try_new(
            self.authority(),
            self.values[index].id(),
            exact(self.backings[index].id()),
            self.units[index].id(),
            StrongCallableDefinitionOwner::GeneratedCallable(self.ensures[index].id()),
            MirObjectValueReadPlanV1::PublishedSingletonRoot {
                object: exact(self.objects[index].id()),
            },
            &mut meter(),
        )
        .unwrap()
    }
    pub fn use_for(
        &self,
        dependency: usize,
        cause: MirExternalInitializationCauseV1,
    ) -> Result<SelectedExternalInitializationUseV1, MirObjectBridgeError> {
        SelectedExternalInitializationUseV1::try_new(
            ConeIdentity::SINGLE_FILE,
            &self.graph,
            self.units[0].id(),
            ConeIdentity::CORE,
            self.units[dependency].id(),
            cause,
            &mut meter(),
        )
    }
}
fn name(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
fn site(cone: ConeIdentity, owner: Option<PersistentTypeId>) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        cone,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(
            owner.into_iter().map(DefinitionOwnerAtom::Type).collect(),
        ),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
pub(super) fn exact(nominal: PersistentTypeId) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}
fn no_bases() -> MirBaseAndInterfacesV1 {
    MirBaseAndInterfacesV1 {
        base: MirBaseClassV1::None,
        interfaces: vec![],
    }
}
fn reference_facts() -> MirTypeFactsV1 {
    MirTypeFactsV1::try_new(
        MirValueKindV1::Reference,
        MirGcKindV1::ContainsManagedReferences,
    )
    .unwrap()
}
