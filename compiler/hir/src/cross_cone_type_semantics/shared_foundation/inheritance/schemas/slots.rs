//! Join implementation claims to the shared declaration's actual selections.

use super::*;
use crate::{
    CallableDeclarationRecordV1, DeclaredVisibilityV1,
    InheritanceCallableDeclarationV1 as Declaration, InheritanceCallableSignatureV1,
    InheritanceSlotSourceSemanticAuthority, InheritanceSourceSlotSelectionV1 as Selection,
};
use scoop_identity::CallableTemplateOrigin;

mod authority;
mod declarations;
mod signatures;

pub(super) fn validate<'a>(
    current: CheckedSharedTypeFoundationV1<'a>,
    dependencies: &[CheckedSharedTypeFoundationV1<'a>],
    sources: &Context<'_>,
    schemas: &mut SchemaDeclarations<'a>,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
) -> Result<(), Error> {
    let mut data = Data::default();
    let metadata_dependencies = dependencies
        .iter()
        .map(|dependency| dependency.metadata)
        .collect::<Vec<_>>();
    for provider in std::iter::once(current).chain(dependencies.iter().copied()) {
        declarations::collect(&mut data, schemas, provider)?;
    }
    for provider in std::iter::once(current).chain(dependencies.iter().copied()) {
        for nominal in provider.section.inheritance().records() {
            let choices = schemas
                .selections
                .get(&nominal.owner())
                .ok_or(Error::SlotSelectionInventory(nominal.owner()))?;
            let slots = nominal.slots().records();

            if choices.records().len() != slots.len()
                || !choices
                    .records()
                    .iter()
                    .map(|choice| choice.slot())
                    .eq(slots.iter().map(|slot| slot.slot()))
            {
                return Err(Error::SlotSelectionInventory(nominal.owner()));
            }
            for slot in slots {
                signatures::project(
                    &mut data,
                    current.metadata,
                    &metadata_dependencies,
                    nominal.owner(),
                    slot.declaration(),
                    slot.signature().receiver(),
                )?;
                let target = slot.implementation().target();
                signatures::project(
                    &mut data,
                    current.metadata,
                    &metadata_dependencies,
                    nominal.owner(),
                    target.declaration(),
                    target.signature().receiver(),
                )?;
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
                    .validate_slot_source_contract(nominal.owner(), slot, &replay)
                    .map_err(|error| Error::SlotContracts(Box::new(error)))?;
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct Data<'a> {
    members: BTreeMap<Declaration, Member<'a>>,
    origins: BTreeSet<ExportDefinitionSourceV1>,
    signatures: BTreeMap<(Declaration, PersistentExactTypeId), InheritanceCallableSignatureV1>,
    exacts: BTreeMap<PersistentExactTypeId, Arc<ExactTypeKey>>,
}

struct Member<'a> {
    source: &'a CallableDeclarationRecordV1,
    access: DeclarationAccessSourceV1,
}

fn selection_error(owner: PersistentExactTypeId, slot: PersistentDispatchSlotId) -> Error {
    Error::SlotSelection { owner, slot }
}
