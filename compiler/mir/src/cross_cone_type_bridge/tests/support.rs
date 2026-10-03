use super::*;
use crate::{CanonicalMirFoundation, DecodedMirFoundation, OdrFreeMirFoundation};
use scoop_hir::{CanonicalHirFoundation, DecodedHirFoundation};
use scoop_identity::{
    CallableMaterialization, CallableMaterializationContext, CallableTemplateOwner,
    CanonicalIdentifier, ConeIdentity, DeclarationScope, DefinitionOwnerChain, PackagePath,
    PendingIdentityValidation, PersistentFunctionId, SourceDeclarationSite,
};

type NominalRecord = CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>;
type ExactRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;

pub(in crate::cross_cone_type_bridge) struct Fixture {
    pub graph: ValidatedIdentityGraph,
    pub foundation: OdrFreeMirFoundation,
    pub empty: NominalRecord,
    pub other: NominalRecord,
    pub class: NominalRecord,
    pub interface: NominalRecord,
    pub other_interface: NominalRecord,
    pub enumeration: NominalRecord,
    pub object: NominalRecord,
    pub backing: crate::GeneratedNominalRecord,
    pub frame: crate::GeneratedNominalRecord,
    pub payload: ExactRecord,
    pub fields: [FieldRecord; 2],
    pub boxed: crate::BoxedValueIdentity,
    pub step: crate::CoroutineStepIdentity,
    pub slot: crate::CoroutineSlotIdentity,
    pub variant: CborIdentityRecord<PersistentEnumVariantId, EnumVariantIdentityKey>,
    pub variant_field: CborIdentityRecord<PersistentEnumVariantFieldId, EnumVariantFieldKey>,
}
impl Fixture {
    pub fn new() -> Self {
        Self::with_source("Empty", SourceNominalKind::Struct)
    }
    pub fn with_source(name: &str, kind: SourceNominalKind) -> Self {
        Self::with_source_provider(name, kind, ConeIdentity::SINGLE_FILE)
    }
    pub fn with_source_provider(
        name: &str,
        kind: SourceNominalKind,
        provider: ConeIdentity,
    ) -> Self {
        let empty = nominal_in(name, kind, provider);
        let other = nominal("Other", SourceNominalKind::Struct);
        let class = nominal("Base", SourceNominalKind::Class);
        let interface = nominal("Interface", SourceNominalKind::Interface);
        let other_interface = nominal("OtherInterface", SourceNominalKind::Interface);
        let enumeration = nominal("Choice", SourceNominalKind::Enum);
        let object = nominal("Registry", SourceNominalKind::Object);
        let backing = CborIdentityRecord::from_key(GeneratedNominalKey::ObjectBackingClass {
            object: object.id(),
        })
        .unwrap();
        let frame_owner: CborIdentityRecord<PersistentFunctionId, _> =
            CborIdentityRecord::from_key(SourceDeclarationKey::function(
                site(),
                CanonicalIdentifier::new("suspendedOwner").unwrap(),
                0,
                None,
                vec![],
            ))
            .unwrap();
        let frame = CborIdentityRecord::from_key(GeneratedNominalKey::CoroutineFrame {
            source_callable: CallableMaterialization::new(
                CallableTemplateOwner::Function(frame_owner.id()),
                CallableMaterializationContext::NoSubstitution,
            ),
        })
        .unwrap();
        let payload = exact(empty.id());
        let field_owner = if kind == SourceNominalKind::Struct {
            empty.key()
        } else {
            other.key()
        };
        let fields = ["second", "first"].map(|name| {
            CborIdentityRecord::from_key(
                FieldIdentityKey::source_declared(
                    field_owner,
                    CanonicalIdentifier::new(name).unwrap(),
                )
                .unwrap(),
            )
            .unwrap()
        });
        let boxed = crate::BoxedValueIdentity::for_source_nominal(&payload).unwrap();
        let step = crate::CoroutineStepIdentity::new(&payload, None).unwrap();
        let slot = crate::CoroutineSlotIdentity::new(&payload, None).unwrap();
        let variant = CborIdentityRecord::from_key(
            EnumVariantIdentityKey::source(
                enumeration.key(),
                CanonicalIdentifier::new("Value").unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let variant_field = CborIdentityRecord::from_key(EnumVariantFieldKey::new(
            variant.id(),
            EnumVariantFieldSelector::Positional {
                declaration_index: 0,
            },
        ))
        .unwrap();
        let sources = vec![
            empty.clone(),
            other.clone(),
            class.clone(),
            interface.clone(),
            other_interface.clone(),
            enumeration.clone(),
            object.clone(),
        ];
        let mut hir = CanonicalHirFoundation::empty();
        hir.set_types(sources.clone()).unwrap();
        hir.set_exact_types(
            sources
                .iter()
                .map(|record| exact(record.id()))
                .chain([exact(backing.id())])
                .collect(),
        )
        .unwrap();
        hir.set_generated_types(vec![backing.clone()]).unwrap();
        hir.set_functions(vec![frame_owner]).unwrap();
        hir.set_fields(fields.to_vec()).unwrap();
        hir.set_enum_variants(vec![variant.clone()]).unwrap();
        hir.set_enum_variant_fields(vec![variant_field.clone()])
            .unwrap();
        let generated = vec![
            boxed.generated_type_record().clone(),
            step.generated_type_record().clone(),
            slot.generated_type_record().clone(),
            frame.clone(),
        ];
        let mut mir = CanonicalMirFoundation::empty();
        mir.set_exact_types(generated.iter().map(|record| exact(record.id())).collect())
            .unwrap();
        mir.set_generated_types(generated).unwrap();
        mir.set_fields(vec![boxed.payload_field_record().clone()])
            .unwrap();
        mir.set_enum_variants(vec![
            step.completed_variant_record().clone(),
            step.suspended_variant_record().clone(),
            slot.empty_variant_record().clone(),
            slot.value_variant_record().clone(),
        ])
        .unwrap();
        mir.set_enum_variant_fields(vec![
            step.completed_payload_record().clone(),
            slot.value_payload_record().clone(),
        ])
        .unwrap();
        let hir: DecodedHirFoundation = decode_canonical(&encode(&hir).unwrap()).unwrap();
        let mir: DecodedMirFoundation = decode_canonical(&encode(&mir).unwrap()).unwrap();
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_authority(ConeIdentity::SINGLE_FILE)
            .unwrap();
        if provider != ConeIdentity::SINGLE_FILE {
            pending.register_authority(provider).unwrap();
        }
        hir.register_identities(&mut pending).unwrap();
        mir.register_identities(&mut pending).unwrap();
        hir.resolve_identities(&mut pending).unwrap();
        mir.resolve_identities(&mut pending).unwrap();
        let mut graph = pending.finish().unwrap();
        let foundation =
            OdrFreeMirFoundation::from_validated(mir.validate(&mut graph).unwrap()).unwrap();
        Self {
            graph,
            foundation,
            empty,
            other,
            class,
            interface,
            other_interface,
            enumeration,
            object,
            backing,
            frame,
            payload,
            fields,
            boxed,
            step,
            slot,
            variant,
            variant_field,
        }
    }
    pub fn authority(&self) -> MirTypeBridgeAuthority<'_> {
        MirTypeBridgeAuthority {
            identities: &self.graph,
            foundation: &self.foundation,
        }
    }
    pub fn source(
        &self,
        nominal: PersistentTypeId,
        facts: MirTypeFactsV1,
        representation: MirTypeRepresentationV1,
    ) -> Result<ParamFreeMirTypeExportV1, MirTypeBridgeError> {
        ParamFreeMirTypeExportV1::try_new(
            self.authority(),
            exact(nominal).id(),
            MirTypeOriginV1::SourceNominal(nominal),
            facts,
            representation,
            no_bases(),
        )
    }
    pub fn empty_export(&self) -> ParamFreeMirTypeExportV1 {
        self.source(
            self.empty.id(),
            facts(MirValueKindV1::ZeroSizedValue, MirGcKindV1::GcFree),
            MirTypeRepresentationV1::Struct {
                fields: vec![],
                c_layout: MirTypeCLayoutPolicyV1::Ordinary,
                interior_mutable: false,
            },
        )
        .unwrap()
    }
    pub fn boxed_export(&self) -> ParamFreeMirTypeExportV1 {
        let nominal = self.boxed.generated_type_record().id();
        ParamFreeMirTypeExportV1::try_new(
            self.authority(),
            exact(nominal).id(),
            MirTypeOriginV1::GeneratedNominal {
                nominal,
                role: self.boxed.generated_type_record().key().clone(),
            },
            facts(
                MirValueKindV1::Reference,
                MirGcKindV1::ContainsManagedReferences,
            ),
            MirTypeRepresentationV1::BoxedValue {
                payload: MirRepresentationFieldV1 {
                    field: self.boxed.payload_field_record().id(),
                    value: self.payload.id(),
                },
            },
            no_bases(),
        )
        .unwrap()
    }
    pub fn step_export(&self) -> ParamFreeMirTypeExportV1 {
        let nominal = self.step.generated_type_record().id();
        ParamFreeMirTypeExportV1::try_new(
            self.authority(),
            exact(nominal).id(),
            MirTypeOriginV1::GeneratedNominal {
                nominal,
                role: self.step.generated_type_record().key().clone(),
            },
            facts(MirValueKindV1::NonZeroValue, MirGcKindV1::GcFree),
            MirTypeRepresentationV1::CoroutineStep {
                variants: vec![
                    MirRepresentationVariantV1 {
                        variant: self.step.completed_variant_record().id(),
                        fields: vec![MirRepresentationVariantFieldV1 {
                            field: self.step.completed_payload_record().id(),
                            value: self.payload.id(),
                        }],
                        gc: MirGcKindV1::GcFree,
                    },
                    MirRepresentationVariantV1 {
                        variant: self.step.suspended_variant_record().id(),
                        fields: vec![],
                        gc: MirGcKindV1::GcFree,
                    },
                ],
            },
            no_bases(),
        )
        .unwrap()
    }
    pub fn slot_export(&self) -> ParamFreeMirTypeExportV1 {
        let nominal = self.slot.generated_type_record().id();
        ParamFreeMirTypeExportV1::try_new(
            self.authority(),
            exact(nominal).id(),
            MirTypeOriginV1::GeneratedNominal {
                nominal,
                role: self.slot.generated_type_record().key().clone(),
            },
            facts(MirValueKindV1::NonZeroValue, MirGcKindV1::GcFree),
            MirTypeRepresentationV1::CoroutineSlot {
                variants: vec![
                    MirRepresentationVariantV1 {
                        variant: self.slot.empty_variant_record().id(),
                        fields: vec![],
                        gc: MirGcKindV1::GcFree,
                    },
                    MirRepresentationVariantV1 {
                        variant: self.slot.value_variant_record().id(),
                        fields: vec![MirRepresentationVariantFieldV1 {
                            field: self.slot.value_payload_record().id(),
                            value: self.payload.id(),
                        }],
                        gc: MirGcKindV1::GcFree,
                    },
                ],
            },
            no_bases(),
        )
        .unwrap()
    }
}
pub(super) fn nominal(name: &str, kind: SourceNominalKind) -> NominalRecord {
    nominal_in(name, kind, ConeIdentity::SINGLE_FILE)
}
fn nominal_in(name: &str, kind: SourceNominalKind, provider: ConeIdentity) -> NominalRecord {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        0,
    ))
    .unwrap()
}
fn site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
pub(super) fn exact(nominal: PersistentTypeId) -> ExactRecord {
    CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal)).unwrap()
}
pub(super) fn no_bases() -> MirBaseAndInterfacesV1 {
    MirBaseAndInterfacesV1 {
        base: MirBaseClassV1::None,
        interfaces: vec![],
    }
}
pub(super) fn facts(kind: MirValueKindV1, gc: MirGcKindV1) -> MirTypeFactsV1 {
    MirTypeFactsV1::try_new(kind, gc).unwrap()
}
