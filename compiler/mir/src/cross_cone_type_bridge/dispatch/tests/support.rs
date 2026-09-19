use super::*;
use scoop_hir::{CanonicalHirFoundation, DecodedHirFoundation};

type Nominal = CborIdentityRecord<PersistentTypeId, SourceDeclarationKey>;
type Method = CborIdentityRecord<PersistentFunctionId, SourceDeclarationKey>;
type Slot = CborIdentityRecord<PersistentDispatchSlotId, DispatchSlotKey>;
pub(super) const UNIT: usize = 0;
pub(super) const BASE: usize = 1;
pub(super) const DERIVED: usize = 2;
pub(super) const OTHER: usize = 3;
pub(super) const ROOT: usize = 4;
pub(super) const LEFT: usize = 5;
pub(super) const RIGHT: usize = 6;
pub(super) const DIAMOND: usize = 7;
pub(super) const VALUE: usize = 8;

pub(super) struct Fixture {
    pub graph: ValidatedIdentityGraph,
    pub foundation: crate::OdrFreeMirFoundation,
    pub types: CanonicalParamFreeMirTypeExportsV1,
    pub callables: CanonicalMirCallableBindingsV1,
    pub source: Vec<Nominal>,
    pub methods: Vec<Method>,
    pub slots: Vec<Slot>,
    pub boxing: Vec<crate::GeneratedCallableRecord>,
}
impl Fixture {
    pub fn new() -> Self {
        Self::with_dependency_cone(false)
    }
    pub fn with_dependency_cone(separate: bool) -> Self {
        let source: Vec<_> = [
            ("Unit", SourceNominalKind::Struct),
            ("Base", SourceNominalKind::Class),
            ("Derived", SourceNominalKind::Class),
            ("Other", SourceNominalKind::Class),
            ("Root", SourceNominalKind::Interface),
            ("Left", SourceNominalKind::Interface),
            ("Right", SourceNominalKind::Interface),
            ("Diamond", SourceNominalKind::Interface),
            ("Value", SourceNominalKind::Struct),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (name, kind))| {
            let provider = if separate && matches!(index, UNIT | BASE | ROOT..=DIAMOND) {
                ConeIdentity::CORE
            } else {
                ConeIdentity::SINGLE_FILE
            };
            nominal(name, kind, provider)
        })
        .collect();
        let methods: Vec<_> = [
            (BASE, "virtual"),
            (DERIVED, "override"),
            (OTHER, "unrelated"),
            (ROOT, "required"),
            (ROOT, "default"),
            (DERIVED, "implementation"),
            (VALUE, "implementation"),
        ]
        .into_iter()
        .map(|(owner, method)| {
            CborIdentityRecord::from_key(SourceDeclarationKey::function(
                site(Some(source[owner].id()), source[owner].key().origin()),
                name(method),
                0,
                None,
                vec![],
            ))
            .unwrap()
        })
        .collect();
        let slots = vec![
            CborIdentityRecord::from_key(DispatchSlotKey::virtual_method(methods[0].id())).unwrap(),
            CborIdentityRecord::from_key(DispatchSlotKey::interface_method(methods[3].id()))
                .unwrap(),
            CborIdentityRecord::from_key(DispatchSlotKey::interface_method(methods[4].id()))
                .unwrap(),
        ];
        let boxing: Vec<_> = [ROOT, LEFT]
            .into_iter()
            .flat_map(|interface| {
                slots[1..]
                    .iter()
                    .map(|slot| {
                        CborIdentityRecord::from_key(GeneratedCallableKey::BoxingAdjust {
                            slot: slot.id(),
                            payload: exact(source[VALUE].id()),
                            interface: exact(source[interface].id()),
                        })
                        .unwrap()
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut hir = CanonicalHirFoundation::empty();
        hir.set_types(source.clone()).unwrap();
        hir.set_exact_types(
            source
                .iter()
                .map(|ty| CborIdentityRecord::from_key(ExactTypeKey::Nominal(ty.id())).unwrap())
                .collect(),
        )
        .unwrap();
        hir.set_functions(methods.clone()).unwrap();
        hir.set_dispatch_slots(slots.clone()).unwrap();
        let signatures: Vec<_> = [BASE, DERIVED, OTHER, ROOT, ROOT, DERIVED, VALUE]
            .into_iter()
            .map(|receiver| sig(exact(source[receiver].id()), exact(source[UNIT].id())))
            .collect();
        let mut mir = crate::CanonicalMirFoundation::empty();
        mir.set_generated_callables(boxing.clone()).unwrap();
        mir.set_callable_signatures(
            methods
                .iter()
                .zip(&signatures)
                .map(|(method, signature)| {
                    crate::CallableSignatureRecord::new(
                        crate::CallableSignatureSubject::strong(
                            StrongCallableDefinitionOwner::Function(method.id()).callable_owner(),
                        ),
                        signature.clone(),
                    )
                })
                .chain(boxing.iter().map(|generated| {
                    crate::CallableSignatureRecord::new(
                        crate::CallableSignatureSubject::strong(
                            StrongCallableDefinitionOwner::GeneratedCallable(generated.id())
                                .callable_owner(),
                        ),
                        match generated.key() {
                            GeneratedCallableKey::BoxingAdjust { interface, .. } => {
                                sig(*interface, exact(source[UNIT].id()))
                            }
                            _ => unreachable!(),
                        },
                    )
                }))
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
        if separate {
            pending.register_authority(ConeIdentity::CORE).unwrap();
        }
        hir.register_identities(&mut pending).unwrap();
        mir.register_identities(&mut pending).unwrap();
        hir.resolve_identities(&mut pending).unwrap();
        mir.resolve_identities(&mut pending).unwrap();
        let mut graph = pending.finish().unwrap();
        let foundation = crate::OdrFreeMirFoundation::from_validated(
            mir.validate(&mut graph, &mut meter()).unwrap(),
        )
        .unwrap();
        let type_authority = MirTypeBridgeAuthority {
            identities: &graph,
            foundation: &foundation,
        };
        let records = source
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let (kind, representation) = match index {
                    UNIT => (
                        MirValueKindV1::ZeroSizedValue,
                        MirTypeRepresentationV1::Intrinsic(MirParamFreeIntrinsicV1::Unit),
                    ),
                    VALUE => (
                        MirValueKindV1::ZeroSizedValue,
                        MirTypeRepresentationV1::Struct {
                            fields: vec![],
                            c_layout: MirTypeCLayoutPolicyV1::Ordinary,
                            interior_mutable: false,
                        },
                    ),
                    BASE..=OTHER => (
                        MirValueKindV1::Reference,
                        MirTypeRepresentationV1::Class {
                            kind: if index == BASE {
                                MirClassKindV1::Open
                            } else {
                                MirClassKindV1::Final
                            },
                            declared_fields: vec![],
                        },
                    ),
                    _ => (
                        MirValueKindV1::Reference,
                        MirTypeRepresentationV1::Interface,
                    ),
                };
                ParamFreeMirTypeExportV1::try_new(
                    type_authority,
                    exact(source.id()),
                    MirTypeOriginV1::SourceNominal(source.id()),
                    facts(kind),
                    representation,
                    MirBaseAndInterfacesV1 {
                        base: MirBaseClassV1::None,
                        interfaces: vec![],
                    },
                )
                .unwrap()
            })
            .collect();
        let types = CanonicalParamFreeMirTypeExportsV1::try_new(records).unwrap();
        let mut fixture = Self {
            graph,
            foundation,
            types,
            callables: CanonicalMirCallableBindingsV1::try_new(vec![]).unwrap(),
            source,
            methods,
            slots,
            boxing,
        };
        for (index, base, interfaces) in [
            (DERIVED, Some(BASE), vec![DIAMOND]),
            (LEFT, None, vec![ROOT]),
            (RIGHT, None, vec![ROOT]),
            (DIAMOND, None, vec![LEFT, RIGHT]),
            (VALUE, None, vec![LEFT]),
        ] {
            fixture.set_edges(index, base, &interfaces);
        }
        let authority = MirCallableBridgeAuthority {
            identities: &fixture.graph,
            foundation: &fixture.foundation,
            types: &fixture.types,
        };
        let mut bindings: Vec<_> = fixture
            .methods
            .iter()
            .zip(signatures)
            .enumerate()
            .map(|(index, (method, signature))| {
                let origin = MirCallableOriginV1::Function(method.id());
                let signature = MirBridgeCallableSignatureV1::new(
                    signature,
                    if index == 6 {
                        crate::GcEffect::NoGc
                    } else {
                        crate::GcEffect::Managed
                    },
                );
                ParamFreeMirCallableBindingV1::try_new(
                    authority,
                    origin.clone(),
                    origin.implementation(),
                    signature.clone(),
                    signature,
                    if index == 3 {
                        MirCallableLoweringRoleV1::PureVirtualTrap {
                            slot: fixture.slots[1].id(),
                        }
                    } else {
                        MirCallableLoweringRoleV1::Ordinary
                    },
                )
                .unwrap()
            })
            .collect();
        for (index, target) in [6, 4, 6, 4].into_iter().enumerate() {
            let generated = &fixture.boxing[index];
            let origin = MirCallableOriginV1::Generated {
                callable: generated.id(),
                role: generated.key().clone(),
            };
            bindings.push(
                ParamFreeMirCallableBindingV1::try_new(
                    authority,
                    origin.clone(),
                    origin.implementation(),
                    bindings[target].lowered_signature().clone(),
                    MirBridgeCallableSignatureV1::new(
                        sig(
                            fixture.exact(if index < 2 { ROOT } else { LEFT }),
                            fixture.exact(UNIT),
                        ),
                        crate::GcEffect::Managed,
                    ),
                    MirCallableLoweringRoleV1::BoxingAdjust {
                        target: fixture.target(target),
                    },
                )
                .unwrap(),
            );
        }
        fixture.callables = CanonicalMirCallableBindingsV1::try_new(bindings).unwrap();
        fixture
    }
    pub fn exact(&self, index: usize) -> PersistentExactTypeId {
        exact(self.source[index].id())
    }
    pub fn target(&self, index: usize) -> StrongCallableDefinitionOwner {
        StrongCallableDefinitionOwner::Function(self.methods[index].id())
    }
    pub fn authority(&self) -> MirDispatchSchemaAuthority<'_> {
        MirDispatchSchemaAuthority {
            identities: &self.graph,
            types: &self.types,
            callables: &self.callables,
        }
    }
    pub fn set_edges(&mut self, index: usize, base: Option<usize>, interfaces: &[usize]) {
        let mut edges: Vec<_> = interfaces.iter().map(|i| self.exact(*i)).collect();
        edges.sort();
        let previous = self.types.get(self.exact(index)).unwrap();
        let replacement = ParamFreeMirTypeExportV1::try_new(
            MirTypeBridgeAuthority {
                identities: &self.graph,
                foundation: &self.foundation,
            },
            previous.exact(),
            previous.origin().clone(),
            previous.facts(),
            previous.representation().clone(),
            MirBaseAndInterfacesV1 {
                base: base.map_or(MirBaseClassV1::None, |i| {
                    MirBaseClassV1::Base(self.exact(i))
                }),
                interfaces: edges,
            },
        )
        .unwrap();
        self.types = CanonicalParamFreeMirTypeExportsV1::try_new(
            self.types
                .records()
                .iter()
                .map(|record| {
                    if record.exact() == replacement.exact() {
                        replacement.clone()
                    } else {
                        record.clone()
                    }
                })
                .collect(),
        )
        .unwrap();
    }
}
fn name(value: &str) -> CanonicalIdentifier {
    CanonicalIdentifier::new(value).unwrap()
}
fn site(owner: Option<PersistentTypeId>, provider: ConeIdentity) -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        provider,
        PackagePath::root(),
        DefinitionOwnerChain::from_outer_to_inner(
            owner.into_iter().map(DefinitionOwnerAtom::Type).collect(),
        ),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}
fn nominal(value: &str, kind: SourceNominalKind, provider: ConeIdentity) -> Nominal {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        site(None, provider),
        name(value),
        kind,
        0,
    ))
    .unwrap()
}
fn exact(nominal: PersistentTypeId) -> PersistentExactTypeId {
    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(nominal)).unwrap()
}
fn sig(receiver: PersistentExactTypeId, result: PersistentExactTypeId) -> ExactCallableSignature {
    ExactCallableSignature::new(Effect::Ordinary, Some(receiver), vec![], result)
}
fn facts(kind: MirValueKindV1) -> MirTypeFactsV1 {
    MirTypeFactsV1::try_new(
        kind,
        if kind == MirValueKindV1::Reference {
            MirGcKindV1::ContainsManagedReferences
        } else {
            MirGcKindV1::GcFree
        },
    )
    .unwrap()
}
