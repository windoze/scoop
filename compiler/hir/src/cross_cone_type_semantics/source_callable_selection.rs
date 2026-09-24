//! Temporary borrowed selection from shared declarations, never a wire authority.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{
    CallableTemplateOrigin as Origin, ConeIdentity, DependencyCallableDeclarationId as Declaration,
    PersistentPropertyId, ValidatedIdentityGraph,
};
use scoop_wire::{BudgetMeter, WirePath};

use crate::{
    CallableDeclarationRecordV1, CallableModalityV1, CrossConeHirInterfaceSectionV1,
    CrossConeTypeSemanticsSectionV1, InheritanceCallableDeclarationV1, ProtectedDeclarationRefV1,
    SharedTypeMetadataError as Error,
};

mod constructors;
mod identity;
mod ordinary;
mod properties;
mod signatures;
pub use constructors::select_param_free_source_constructors;
pub use ordinary::select_ordinary_source_callables;

/// Selects only source functions/accessors with a closed machine signature.
/// The result borrows the shared declarations; it grants no import capability.
pub fn select_param_free_source_callables<'a>(
    provider: ConeIdentity,
    public: &'a CrossConeHirInterfaceSectionV1,
    types: &CrossConeTypeSemanticsSectionV1,
    identities: &ValidatedIdentityGraph,
    meter: &mut BudgetMeter,
) -> Result<BTreeMap<Declaration, &'a CallableDeclarationRecordV1>, Error> {
    let mut selection = Selection {
        provider,
        public,
        identities,
        signatures: signatures::MaterializableSignatures::new(public, meter)?,
        required: BTreeMap::new(),
        properties: BTreeSet::new(),
        meter,
    };
    for source in public.callable_interfaces().records() {
        selection.insert(source.declaration())?;
    }
    for nominal in types.inheritance().records() {
        selection.meter.charge_work(1, &WirePath::root())?;
        for member in nominal.protected_members().values() {
            selection.meter.charge_work(1, &WirePath::root())?;
            match member {
                ProtectedDeclarationRefV1::Callable(reference) => {
                    selection.insert(reference.declaration())?
                }
                ProtectedDeclarationRefV1::Property(id) => selection.require_property(*id)?,
                ProtectedDeclarationRefV1::Constructor(_)
                | ProtectedDeclarationRefV1::NestedNominal(_) => continue,
            }
        }
        for slot in nominal.slots().records() {
            selection.insert(origin(slot.declaration()))?;
            if let Some(target) = slot.implementation().target() {
                selection.insert(origin(target.declaration()))?;
            }
        }
    }
    // An abstract override owns its trap even when the inherited slot keeps
    // the original declaration and has no concrete selected target.
    for source in public.callable_interfaces().all_declarations() {
        selection.meter.charge_work(1, &WirePath::root())?;
        if source.modality() == CallableModalityV1::Abstract
            && !source.slot_relations().is_empty()
            && selection.materialized_owner(source, types)?
        {
            selection.insert(source.declaration())?;
        }
    }
    selection.properties()?;
    Ok(selection.required)
}

struct Selection<'a, 'i, 'm> {
    provider: ConeIdentity,
    public: &'a CrossConeHirInterfaceSectionV1,
    identities: &'i ValidatedIdentityGraph,
    signatures: signatures::MaterializableSignatures<'a>,
    required: BTreeMap<Declaration, &'a CallableDeclarationRecordV1>,
    properties: BTreeSet<PersistentPropertyId>,
    meter: &'m mut BudgetMeter,
}

impl Selection<'_, '_, '_> {
    fn insert(&mut self, origin: Origin) -> Result<(), Error> {
        let declaration = match origin {
            Origin::Function(id) => Declaration::Function(id),
            Origin::Accessor(id) => Declaration::PropertyAccessor(id),
            Origin::GenericFunction(_) | Origin::Constructor(_) | Origin::VariantConstructor(_) => {
                return Ok(());
            }
        };
        lookup(self.required.len(), self.meter)?;
        if self.required.contains_key(&declaration)
            || !identity::is_local(self.provider, self.identities, origin, self.meter)?
        {
            return Ok(());
        }
        lookup(
            self.public.callable_interfaces().declaration_count(),
            self.meter,
        )?;
        let source = self
            .public
            .callable_interfaces()
            .declaration(origin)
            .ok_or(Error::CallableContract(origin))?;
        if !self.signatures.callable(source, self.meter)? {
            return Ok(());
        }
        self.meter
            .check_table_entries(self.required.len() as u64 + 1, &WirePath::root())?;
        self.meter.charge_collection_slots(1, &WirePath::root())?;
        lookup(self.required.len(), self.meter)?;
        self.required.insert(declaration, source);
        Ok(())
    }
}

fn origin(declaration: InheritanceCallableDeclarationV1) -> Origin {
    match declaration {
        InheritanceCallableDeclarationV1::Function(id) => Origin::Function(id),
        InheritanceCallableDeclarationV1::Getter(id)
        | InheritanceCallableDeclarationV1::Setter(id) => Origin::Accessor(id),
    }
}

fn lookup(length: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    Ok(meter.charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())?)
}
