//! Resolve implementations from shared declarations before checking wire claims.

use super::*;
use crate::{
    CallableDeclarationRecordV1, CallableModalityV1, DeclaredVisibilityV1,
    InheritanceCallableDeclarationV1 as Declaration, InheritanceCallableSignatureV1,
    InheritanceSlotContractV1, InheritanceSlotSourceSemanticAuthority,
    InheritanceSourceSlotSelectionV1 as Selection,
};
use scoop_identity::{CallableTemplateOrigin, ExactCallableSignature, SourceDeclarationKind};

mod authority;
mod declarations;
mod interfaces;
mod selection;
mod signatures;

pub(super) fn validate<'a>(
    current: CheckedSharedTypeFoundationV1<'a>,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    sources: &Context<'_>,
    schemas: &mut SchemaDeclarations<'a>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let mut data = Data::default();
    for provider in std::iter::once(current).chain(dependencies.iter().copied()) {
        declarations::collect(&mut data, schemas, provider, meter)?;
    }
    for provider in std::iter::once(current).chain(dependencies.iter().copied()) {
        for nominal in provider.section.inheritance().records() {
            for slot in nominal.slots().records() {
                let chosen =
                    selection::select(&data, schemas, graph, nominal.owner(), slot, meter)?;
                meter.charge_collection_slots(1, &WirePath::root())?;
                contracts::lookup(data.selections.len(), meter)?;
                data.selections
                    .insert((nominal.owner(), slot.slot()), chosen);
                signatures::project(&mut data, slot.declaration(), dependencies, meter)?;
                if let Some(target) = slot.implementation().target() {
                    signatures::project(&mut data, target.declaration(), dependencies, meter)?;
                }
            }
        }
    }
    let types = MetadataTypes {
        current: current.metadata,
        dependencies,
    };
    let unit = types.nominal_exact(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
        meter,
    )?;
    let replay = authority::Replay {
        data: &data,
        schemas,
        sources,
        unit,
    };
    for provider in std::iter::once(current).chain(dependencies.iter().copied()) {
        for nominal in provider.section.inheritance().records() {
            for slot in nominal.slots().records() {
                graph
                    .validate_slot_source_contract(nominal.owner(), slot, &replay, meter)
                    .map_err(|error| Error::SlotContracts(Box::new(error)))?;
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct Data<'a> {
    members: BTreeMap<Declaration, Member<'a>>,
    owners: BTreeMap<PersistentExactTypeId, Vec<Declaration>>,
    origins: BTreeSet<ExportDefinitionSourceV1>,
    signatures: BTreeMap<Declaration, InheritanceCallableSignatureV1>,
    exacts: BTreeMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
    selections: BTreeMap<(PersistentExactTypeId, PersistentDispatchSlotId), Selection>,
}

struct Member<'a> {
    declaration: Declaration,
    owner: PersistentExactTypeId,
    metadata: SharedTypeMetadataV1<'a>,
    source: &'a CallableDeclarationRecordV1,
    key: Arc<SourceDeclarationKey>,
    access: DeclarationAccessSourceV1,
}

impl Data<'_> {
    fn member(
        &self,
        declaration: Declaration,
        meter: &mut BudgetMeter,
    ) -> Result<&Member<'_>, Error> {
        contracts::lookup(self.members.len(), meter)?;
        self.members
            .get(&declaration)
            .ok_or(Error::SlotCallable(declaration))
    }

    fn owner_members(
        &self,
        owner: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<&[Declaration], Error> {
        contracts::lookup(self.owners.len(), meter)?;
        Ok(self
            .owners
            .get(&owner)
            .map(Vec::as_slice)
            .unwrap_or_default())
    }
}

fn selection_error(owner: PersistentExactTypeId, slot: PersistentDispatchSlotId) -> Error {
    Error::SlotSelection { owner, slot }
}

fn selected(member: &Member<'_>) -> Selection {
    match member.source.modality() {
        CallableModalityV1::Abstract => Selection::Abstract,
        CallableModalityV1::InterfaceDefault => Selection::InterfaceDefault(member.declaration),
        CallableModalityV1::Final | CallableModalityV1::Open => {
            Selection::Concrete(member.declaration)
        }
    }
}
