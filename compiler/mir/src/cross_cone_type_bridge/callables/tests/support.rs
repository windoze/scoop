use super::*;
use scoop_hir::{CanonicalHirFoundation, DecodedHirFoundation};

type Function = CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>;
type Constructor = CborIdentityRecord<PersistentConstructorId, SourceDeclarationKey>;
type Accessor = CborIdentityRecord<PersistentPropertyAccessorId, PropertyAccessorKey>;

pub(super) struct Fixture {
    pub graph: ValidatedIdentityGraph,
    pub foundation: crate::OdrFreeMirFoundation,
    pub types: CanonicalParamFreeMirTypeExportsV1,
    pub value: PersistentExactTypeId,
    pub class: PersistentExactTypeId,
    pub interface: PersistentExactTypeId,
    pub unit: PersistentExactTypeId,
    pub boolean: PersistentExactTypeId,
    pub constructors: [Constructor; 2],
    pub method: Function,
    pub abstract_method: Function,
    pub accessors: [Accessor; 2],
    pub slot: CborIdentityRecord<PersistentDispatchSlotId, DispatchSlotKey>,
    pub adjust: crate::GeneratedCallableRecord,
    pub boxing: crate::GeneratedCallableRecord,
    pub ensure: crate::GeneratedCallableRecord,
    pub initializer: crate::GeneratedCallableRecord,
    pub equality: crate::GeneratedCallableRecord,
}
impl Fixture {
    pub fn new() -> Self {
        let value = nominal("Value", SourceNominalKind::Struct);
        let class = nominal("Class", SourceNominalKind::Class);
        let interface = nominal("Interface", SourceNominalKind::Interface);
        let unit = nominal("Unit", SourceNominalKind::Struct);
        let object = nominal("Registry", SourceNominalKind::Object);
        let boolean = nominal("Boolean", SourceNominalKind::Struct);
        let sources = [
            value.clone(),
            class.clone(),
            interface.clone(),
            unit.clone(),
            object.clone(),
            boolean.clone(),
        ];
        let method: Function = CborIdentityRecord::from_key(SourceDeclarationKey::function(
            site(Some(value.id())),
            name("read"),
            0,
            None,
            vec![],
        ))
        .unwrap();
        let abstract_method: Function =
            CborIdentityRecord::from_key(SourceDeclarationKey::function(
                site(Some(interface.id())),
                name("read"),
                0,
                None,
                vec![],
            ))
            .unwrap();
        let constructors = [class.id(), value.id()].map(|owner| {
            CborIdentityRecord::from_key(SourceDeclarationKey::constructor(
                site(Some(owner)),
                vec![],
            ))
            .unwrap()
        });
        let property: CborIdentityRecord<scoop_identity::PersistentPropertyId, _> =
            CborIdentityRecord::from_key(SourceDeclarationKey::property(
                site(Some(class.id())),
                name("value"),
            ))
            .unwrap();
        let accessors = [
            scoop_identity::AccessorRole::Getter,
            scoop_identity::AccessorRole::Setter,
        ]
        .map(|role| {
            CborIdentityRecord::from_key(PropertyAccessorKey::new(
                scoop_identity::PropertyOwner::Property(property.id()),
                role,
            ))
            .unwrap()
        });
        let slot =
            CborIdentityRecord::from_key(DispatchSlotKey::interface_method(abstract_method.id()))
                .unwrap();
        let unit_record =
            CborIdentityRecord::from_key(InitializationUnitKey::Object(object.id())).unwrap();
        let backing = CborIdentityRecord::from_key(GeneratedNominalKey::ObjectBackingClass {
            object: object.id(),
        })
        .unwrap();
        let adjust = CborIdentityRecord::from_key(GeneratedCallableKey::DispatchAdjust {
            slot: slot.id(),
            implementor: exact(value.id()),
            target: CallableMaterialization::new(
                CallableTemplateOwner::Function(method.id()),
                CallableMaterializationContext::NoSubstitution,
            ),
        })
        .unwrap();
        let boxing = CborIdentityRecord::from_key(GeneratedCallableKey::BoxingAdjust {
            slot: slot.id(),
            payload: exact(value.id()),
            interface: exact(interface.id()),
        })
        .unwrap();
        let ensure = CborIdentityRecord::from_key(GeneratedCallableKey::Initialization {
            unit: unit_record.id(),
            role: InitializationCallableRole::Ensure,
        })
        .unwrap();
        let initializer = CborIdentityRecord::from_key(GeneratedCallableKey::Initialization {
            unit: unit_record.id(),
            role: InitializationCallableRole::Initializer,
        })
        .unwrap();
        let equality = CborIdentityRecord::from_key(GeneratedCallableKey::DerivedEquality {
            exact_owner: exact(value.id()),
        })
        .unwrap();
        let mut hir = CanonicalHirFoundation::empty();
        hir.set_types(sources.to_vec()).unwrap();
        hir.set_exact_types(
            sources
                .iter()
                .map(|record| exact_record(record.id()))
                .chain([exact_record(backing.id())])
                .collect(),
        )
        .unwrap();
        hir.set_generated_types(vec![backing.clone()]).unwrap();
        hir.set_functions(vec![method.clone(), abstract_method.clone()])
            .unwrap();
        hir.set_constructors(constructors.to_vec()).unwrap();
        hir.set_properties(vec![property]).unwrap();
        hir.set_property_accessors(accessors.to_vec()).unwrap();
        hir.set_dispatch_slots(vec![slot.clone()]).unwrap();
        hir.set_initialization_units(vec![unit_record]).unwrap();
        let value = exact(value.id());
        let class = exact(class.id());
        let interface = exact(interface.id());
        let unit = exact(unit.id());
        let boolean = exact(boolean.id());
        let signatures = vec![
            (
                StrongCallableDefinitionOwner::GeneratedCallable(equality.id()),
                sig(Some(value), vec![value], boolean),
            ),
            (
                StrongCallableDefinitionOwner::Function(method.id()),
                sig(Some(value), vec![], unit),
            ),
            (
                StrongCallableDefinitionOwner::Function(abstract_method.id()),
                sig(Some(interface), vec![], unit),
            ),
            (
                StrongCallableDefinitionOwner::Constructor(constructors[0].id()),
                sig(Some(class), vec![], unit),
            ),
            (
                StrongCallableDefinitionOwner::Constructor(constructors[1].id()),
                sig(None, vec![], value),
            ),
            (
                StrongCallableDefinitionOwner::PropertyAccessor(accessors[0].id()),
                sig(Some(class), vec![], value),
            ),
            (
                StrongCallableDefinitionOwner::PropertyAccessor(accessors[1].id()),
                sig(Some(class), vec![value], unit),
            ),
            (
                StrongCallableDefinitionOwner::GeneratedCallable(adjust.id()),
                sig(Some(interface), vec![], unit),
            ),
            (
                StrongCallableDefinitionOwner::GeneratedCallable(boxing.id()),
                sig(Some(interface), vec![], unit),
            ),
            (
                StrongCallableDefinitionOwner::GeneratedCallable(ensure.id()),
                sig(None, vec![], unit),
            ),
            (
                StrongCallableDefinitionOwner::GeneratedCallable(initializer.id()),
                sig(None, vec![], unit),
            ),
        ];
        let mut mir = crate::CanonicalMirFoundation::empty();
        mir.set_generated_callables(vec![
            adjust.clone(),
            boxing.clone(),
            ensure.clone(),
            initializer.clone(),
            equality.clone(),
        ])
        .unwrap();
        mir.set_callable_signatures(
            signatures
                .into_iter()
                .map(|(owner, signature)| {
                    crate::CallableSignatureRecord::new(
                        crate::CallableSignatureSubject::strong(owner.callable_owner()),
                        signature,
                    )
                })
                .collect(),
        )
        .unwrap();
        let hir: DecodedHirFoundation = decode_canonical(&encode(&hir).unwrap()).unwrap();
        let mir: crate::DecodedMirFoundation = decode_canonical(&encode(&mir).unwrap()).unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_authority(ConeIdentity::SINGLE_FILE)
            .unwrap();
        hir.register_identities(&mut pending).unwrap();
        mir.register_identities(&mut pending).unwrap();
        hir.resolve_identities(&mut pending).unwrap();
        mir.resolve_identities(&mut pending).unwrap();
        let mut graph = pending.finish().unwrap();
        let foundation =
            crate::OdrFreeMirFoundation::from_validated(mir.validate(&mut graph).unwrap()).unwrap();
        let authority = MirTypeBridgeAuthority {
            identities: &graph,
            foundation: &foundation,
        };
        let shapes = [
            (
                MirValueKindV1::ZeroSizedValue,
                MirTypeRepresentationV1::Struct {
                    fields: vec![],
                    c_layout: MirTypeCLayoutPolicyV1::Ordinary,
                    interior_mutable: false,
                },
            ),
            (
                MirValueKindV1::Reference,
                MirTypeRepresentationV1::Class {
                    kind: MirClassKindV1::Abstract,
                    declared_fields: vec![],
                },
            ),
            (
                MirValueKindV1::Reference,
                MirTypeRepresentationV1::Interface,
            ),
            (
                MirValueKindV1::ZeroSizedValue,
                MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Unit),
            ),
            (
                MirValueKindV1::Reference,
                MirTypeRepresentationV1::Object {
                    backing: exact(backing.id()),
                },
            ),
            (
                MirValueKindV1::NonZeroValue,
                MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Boolean),
            ),
        ];
        let mut records: Vec<_> = sources
            .iter()
            .zip(shapes)
            .map(|(source, (kind, shape))| {
                ParamFreeMirTypeExportV1::try_new(
                    authority,
                    exact(source.id()),
                    MirTypeOriginV1::SourceNominal(source.id()),
                    facts(kind),
                    shape,
                    no_bases(),
                )
                .unwrap()
            })
            .collect();
        records.push(
            ParamFreeMirTypeExportV1::try_new(
                authority,
                exact(backing.id()),
                MirTypeOriginV1::GeneratedNominal {
                    nominal: backing.id(),
                    role: backing.key().clone(),
                },
                facts(MirValueKindV1::Reference),
                MirTypeRepresentationV1::ObjectBacking {
                    declared_fields: vec![],
                },
                no_bases(),
            )
            .unwrap(),
        );
        let types = CanonicalParamFreeMirTypeExportsV1::try_new(records).unwrap();
        Self {
            graph,
            foundation,
            types,
            value,
            class,
            interface,
            unit,
            boolean,
            constructors,
            method,
            abstract_method,
            accessors,
            slot,
            adjust,
            boxing,
            ensure,
            initializer,
            equality,
        }
    }
    pub fn authority(&self) -> MirCallableBridgeAuthority<'_> {
        MirCallableBridgeAuthority {
            identities: &self.graph,
            foundation: &self.foundation,
            types: &self.types,
        }
    }
    pub fn bind(
        &self,
        origin: MirCallableOriginV1,
        semantic: ExactCallableSignature,
        lowered: ExactCallableSignature,
        role: MirCallableLoweringRoleV1,
    ) -> Result<ParamFreeMirCallableBindingV1, MirCallableBridgeError> {
        ParamFreeMirCallableBindingV1::try_new(
            self.authority(),
            origin.clone(),
            origin.implementation().unwrap(),
            signature(semantic),
            signature(lowered),
            role,
        )
    }
    pub fn method_binding(&self) -> ParamFreeMirCallableBindingV1 {
        let signature = sig(Some(self.value), vec![], self.unit);
        self.bind(
            MirCallableOriginV1::Function(self.method.id()),
            signature.clone(),
            signature,
            MirCallableLoweringRoleV1::Ordinary,
        )
        .unwrap()
    }
}
