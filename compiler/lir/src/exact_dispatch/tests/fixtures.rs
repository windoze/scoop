use scoop_identity::*;
use scoop_wire::WireError;

use crate::exact_layout::tests::{Bound, exact, source, unit};
use crate::*;

pub(super) const TARGET: LirTargetProfile = LirTargetProfile::DARWIN_AARCH64;

pub(super) struct DirectFixture {
    pub owner: ExactLayoutExportV1,
    pub target_receiver: ExactLayoutExportV1,
    pub abi: ExactCallableAbiExportV1,
    pub vtable: VtableRecord,
    pub foundation: ConeLirFoundation,
    pub slot: PersistentDispatchSlotId,
    slot_signature: ExactDispatchSlotSignatureV1,
    pub target: StrongCallableDefinitionOwner,
}

impl DirectFixture {
    pub(super) fn new(slot_count: usize) -> Self {
        let owner = named_pointer("Owner");
        Self::build(owner.clone(), owner, slot_count)
    }

    pub(super) fn reference() -> Self {
        Self::build(named_pointer("Owner"), named_pointer("TargetReceiver"), 1)
    }

    fn build(
        owner: ExactLayoutExportV1,
        target_receiver: ExactLayoutExportV1,
        slot_count: usize,
    ) -> Self {
        let result: ExactLayoutExportV1 = unit().into();
        let signature = ExactCallableSignature::new(
            Effect::Ordinary,
            Some(target_receiver.identity().exact()),
            Vec::new(),
            result.identity().exact(),
        );
        let (target, callable_foundation) = callable_foundation("implementation");
        let abi = ExactCallableAbiExportV1::from_signature(
            TARGET,
            target,
            scoop_identity::CanonicalScoopAbiFunctionSignature::new(
                signature,
                vec![
                    target_receiver
                        .value_handle()
                        .unwrap()
                        .scoop_abi_argument(TARGET)
                        .unwrap(),
                ],
                result
                    .value_handle()
                    .unwrap()
                    .scoop_abi_return(TARGET)
                    .unwrap(),
                (ExactCallableProtocolV1::OrdinaryManaged).gc_effect(),
            )
            .unwrap(),
            &callable_foundation,
        )
        .unwrap();
        let identity = TypeDescriptorIdentity::new(
            RuntimeTypeMappingRecord::new(owner.identity().exact()).unwrap(),
            MaterializationRoot::cone_owned(),
        )
        .unwrap();
        let slots = (0..slot_count)
            .map(|_| DispatchEntry {
                callable: CallableRef::Local(LocalFunctionId::from_u32(0)),
            })
            .collect();
        let vtable = VtableRecord::new(&identity, slots).unwrap();
        let foundation =
            table_foundation(vtable.identity_record().clone(), Some(&callable_foundation));
        let declaration = match target {
            StrongCallableDefinitionOwner::Function(function) => function,
            _ => unreachable!(),
        };
        let slot = dispatch_slot(declaration);
        let slot_signature = ExactDispatchSlotSignatureV1::new(
            ExactCallableSignature::new(
                Effect::Ordinary,
                Some(owner.identity().exact()),
                Vec::new(),
                result.identity().exact(),
            ),
            scoop_identity::GcEffect::Managed,
        );
        Self {
            owner,
            target_receiver,
            abi,
            vtable,
            foundation,
            slot,
            slot_signature,
            target,
        }
    }

    pub(super) fn identity_input(&self) -> ExactDispatchEntryInputV1<'_> {
        ExactDispatchEntryInputV1 {
            position: ExactDispatchPositionV1::from_u32(0),
            slot: self.slot,
            slot_signature: self.slot_signature.clone(),
            implementation: ExactDispatchImplementationV1::DirectStrongTarget {
                target: scoop_identity::CallableDefinitionOwner::Strong(self.target),
                receiver: ExactDispatchReceiverAdaptationV1::Identity,
            },
            abi: DispatchCallableAbiV1::Exact {
                record: &self.abi,
                receiver: CallableAbiReceiverInputV1::Receiver(&self.target_receiver),
            },
            slot_receiver_layout: None,
        }
    }

    pub(super) fn reference_input<'a>(
        &'a self,
        layout: Option<&'a ExactLayoutExportV1>,
    ) -> ExactDispatchEntryInputV1<'a> {
        ExactDispatchEntryInputV1 {
            position: ExactDispatchPositionV1::from_u32(0),
            slot: self.slot,
            slot_signature: self.slot_signature.clone(),
            implementation: ExactDispatchImplementationV1::DirectStrongTarget {
                target: scoop_identity::CallableDefinitionOwner::Strong(self.target),
                receiver: ExactDispatchReceiverAdaptationV1::ReferenceDispatch,
            },
            abi: DispatchCallableAbiV1::Exact {
                record: &self.abi,
                receiver: CallableAbiReceiverInputV1::Receiver(&self.target_receiver),
            },
            slot_receiver_layout: layout,
        }
    }

    pub(super) fn local_resolver(
        &self,
    ) -> impl FnMut(CallableRef) -> Result<Option<StrongTypeDispatchCallableRefV2>, WireError> + use<>
    {
        let body = self.abi.definition().semantic_id();
        move |callable| {
            Ok(
                matches!(callable, CallableRef::Local(local) if local.into_u32() == 0)
                    .then_some(StrongTypeDispatchCallableRefV2::Local(body)),
            )
        }
    }
}

pub(super) struct EmptyItable {
    pub table: ItableRecord,
    pub foundation: ConeLirFoundation,
    pub interface: PersistentExactTypeId,
}

pub(super) fn empty_itable() -> EmptyItable {
    let owner = named_pointer("ItableOwner");
    let interface = named_pointer("Interface").identity().exact();
    let identity = TypeDescriptorIdentity::new(
        RuntimeTypeMappingRecord::new(owner.identity().exact()).unwrap(),
        MaterializationRoot::cone_owned(),
    )
    .unwrap();
    let table = ItableRecord::new(
        &identity,
        interface,
        TypeDescriptorRef::Local(TypeDescriptorId::from_raw(la_arena::RawIdx::from_u32(0))),
        Vec::new(),
    )
    .unwrap();
    let foundation = table_foundation(table.identity_record().clone(), None);
    EmptyItable {
        table,
        foundation,
        interface,
    }
}

fn named_pointer(name: &str) -> ExactLayoutExportV1 {
    let bound = Bound::value(exact(&source(name, SourceNominalKind::Class, 0)));
    ExactValueLayoutV1::qualified_pointer(
        bound.identity,
        NichePointerKind::Managed,
        &bound.foundation,
    )
    .unwrap()
    .into()
}

fn callable_foundation(name: &str) -> (StrongCallableDefinitionOwner, ConeLirFoundation) {
    let target = StrongCallableDefinitionOwner::Function(source_function(name));
    let (definition, symbol) = ExternalStrongShapeSubjectV1::Callable(
        scoop_identity::CallableDefinitionOwner::Strong(target),
    )
    .expected_definition(ConeIdentity::SINGLE_FILE)
    .unwrap();
    let definition = CborIdentityRecord::from_key(definition).unwrap();
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    canonical
        .set_callable_bodies(vec![
            RuntimeIdentityRecord::from_key(&CallableBodyKey::strong(target)).unwrap(),
        ])
        .unwrap();
    canonical.set_definition_plans(vec![definition]).unwrap();
    canonical.set_definition_atoms(vec![atom]).unwrap();
    canonical.set_symbol_requests(
        PersistentSymbolRequestTable::new(vec![
            PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap(),
        ])
        .unwrap(),
    );
    (
        target,
        ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap(),
    )
}

fn source_function(name: &str) -> PersistentFunctionId {
    let declaration = SourceDeclarationKey::function(
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        0,
        None,
        Vec::new(),
    );
    PersistentFunctionId::from_source_declaration(&declaration).unwrap()
}

pub(super) fn named_dispatch_slot(name: &str) -> PersistentDispatchSlotId {
    dispatch_slot(source_function(name))
}

fn dispatch_slot(function: PersistentFunctionId) -> PersistentDispatchSlotId {
    PersistentDispatchSlotId::from_key(&DispatchSlotKey::virtual_method(function)).unwrap()
}

fn table_foundation(
    table: CborIdentityRecord<PersistentDispatchTableId, DispatchTableKey>,
    callable: Option<&ConeLirFoundation>,
) -> ConeLirFoundation {
    let (definition, symbol) = ExternalStrongShapeSubjectV1::DispatchTable(table.id())
        .expected_definition(ConeIdentity::SINGLE_FILE)
        .unwrap();
    let definition = CborIdentityRecord::from_key(definition).unwrap();
    let atom = CborIdentityRecord::from_key(ObjectDefinitionAtomKey::new(
        definition.id(),
        DefinitionAtomRole::Primary,
        DefinitionAtomSubkey::Singleton,
    ))
    .unwrap();
    let mut canonical = CanonicalLirFoundation::empty();
    if let Some(callable) = callable {
        canonical
            .set_callable_bodies(callable.callable_bodies().to_vec())
            .unwrap();
    }
    canonical.set_dispatch_tables(vec![table]).unwrap();
    let mut plans = callable
        .into_iter()
        .flat_map(ConeLirFoundation::definition_plans)
        .cloned()
        .collect::<Vec<_>>();
    plans.push(definition);
    canonical.set_definition_plans(plans).unwrap();
    let mut atoms = callable
        .into_iter()
        .flat_map(ConeLirFoundation::definition_atoms)
        .cloned()
        .collect::<Vec<_>>();
    atoms.push(atom);
    canonical.set_definition_atoms(atoms).unwrap();
    let mut symbols = callable
        .into_iter()
        .flat_map(ConeLirFoundation::symbol_requests)
        .copied()
        .collect::<Vec<_>>();
    symbols.push(PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong).unwrap());
    canonical.set_symbol_requests(PersistentSymbolRequestTable::new(symbols).unwrap());
    ConeLirFoundation::try_new(ConeIdentity::SINGLE_FILE, canonical).unwrap()
}
