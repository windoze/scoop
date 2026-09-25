use scoop_wire::{WireError, WirePath};

use super::*;
use crate::{ExternalHirBindingWitnessUse, ExternalHirTargetV1};

pub(super) fn collect(
    output: &crate::Output,
    selected: &crate::SelectedImportedDependencySet,
) -> Result<Vec<ExternalHirBindingWitnessUse>, DependencyCallOccurrenceError> {
    let mut uses = Vec::new();
    occurrences::visit(output, selected, |call| {
        let target = ExternalHirTargetV1::Callable(call.callable().interface().declaration());
        append(&mut uses, target, call.binding())?;
        Ok(())
    })?;
    for constant in selected.constants() {
        let target = ExternalHirTargetV1::Property(scoop_identity::PropertyOwner::Property(
            constant.record().property(),
        ));
        append(&mut uses, target, constant.binding())?;
    }
    for alias in selected.type_aliases() {
        let target = ExternalHirTargetV1::TypeAlias(alias.interface().alias());
        append(&mut uses, target, alias.binding())?;
    }

    uses.sort_unstable();
    uses.dedup();
    Ok(uses)
}

fn append(
    uses: &mut Vec<ExternalHirBindingWitnessUse>,
    target: ExternalHirTargetV1,
    binding: &crate::DirectImportedTargetBinding,
) -> Result<(), WireError> {
    let path = WirePath::root();
    for source in binding.sources() {
        scoop_wire::allocation::try_reserve(uses, 1, &path)?;
        uses.push(ExternalHirBindingWitnessUse::new(
            target,
            crate::ExternalHirBindingWitnessRole::ConcreteSelectedUse,
            source.witness().dependency().clone(),
        ));
    }
    Ok(())
}
