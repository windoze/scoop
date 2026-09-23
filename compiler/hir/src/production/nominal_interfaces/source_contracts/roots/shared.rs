//! Rooted source dependencies from sealed HIR, before any interface is projected.

use super::*;
use scoop_identity::{CallableTemplateOrigin, ConeIdentity, DefinitionOwnerAtom};

mod defaults;
mod identity;
mod index;
mod protocols;
use index::SourceIndex;

pub(in crate::production) struct SharedSourceRoots {
    pub nominals: CanonicalSourceNominalIdsV1,
    pub top_level_callables: BTreeMap<CallableTemplateOrigin, PublicDeclarationOwnerV1>,
    pub top_level_properties: BTreeSet<PropertyDeclarationId>,
}

impl SharedSourceRoots {
    pub(in crate::production) fn from_export_hir(
        export: &ExportHir,
        meter: &mut BudgetMeter,
    ) -> Result<Self, NominalInterfaceBuildError> {
        Self::collect(export, meter).map_err(|error| match error {
            Error::SourceInventory(SourceInventoryError::Resource(error)) => {
                NominalInterfaceBuildError::Resource(error)
            }
            other => NominalInterfaceBuildError::Declarations(other.to_string()),
        })
    }

    fn collect(export: &ExportHir, meter: &mut BudgetMeter) -> Result<Self, Error> {
        let index = super::Index::new(export, meter)?;
        let source_index = SourceIndex::new(export, meter)?;
        let mut nominals = Roots {
            complete_children: true,
            required: BTreeMap::new(),
            pending: Vec::new(),
            field_types: BTreeSet::new(),
            meter,
        };
        let mut sources = SourceRoots::default();
        for local in super::index::public(export) {
            nominals.require(super::index::source(export, local)?, true)?;
        }
        for function in &export.public_surface.functions {
            sources.function(export, *function, &mut nominals)?;
        }
        for property in &export.public_surface.properties {
            sources.property(export, *property, &mut nominals)?;
        }
        loop {
            while let Some(owner) = nominals.expand_next(export, &index)? {
                work(nominals.meter, source_index.members.len())?;
                if let Some(members) = source_index.members.get(&owner) {
                    for member in members {
                        match *member {
                            SourceWork::Callable(id) => {
                                let protocol = source_index.protocol(id, nominals.meter)?;
                                sources.callable(export, protocol.owner, &mut nominals)?;
                            }
                            SourceWork::Property(id) => {
                                sources.property(export, id, &mut nominals)?;
                            }
                        }
                    }
                }
            }
            while let Some(next) = sources.pending.pop() {
                match next {
                    SourceWork::Callable(id) => sources.protocol(
                        export,
                        source_index.protocol(id, nominals.meter)?,
                        &index,
                        &mut nominals,
                    )?,
                    SourceWork::Property(id) => {
                        sources.property_types(export, id, &index, &mut nominals)?;
                    }
                }
            }
            if nominals.pending.is_empty() {
                break;
            }
        }
        let mut required = Vec::new();
        nominals
            .meter
            .try_reserve_collection_slots(&mut required, nominals.required.len(), &WirePath::root())
            .map_err(resource)?;
        required.extend(nominals.required.into_keys());
        Ok(Self {
            nominals: CanonicalSourceNominalIdsV1::try_new(required, nominals.meter)
                .map_err(Error::SourceInventory)?,
            top_level_callables: sources.top_level_callables,
            top_level_properties: sources.top_level_properties,
        })
    }
}

#[derive(Clone, Copy)]
enum SourceWork {
    Callable(CallableTemplateOrigin),
    Property(PropertyId),
}

#[derive(Default)]
struct SourceRoots {
    callables: BTreeSet<CallableTemplateOrigin>,
    properties: BTreeSet<PropertyDeclarationId>,
    defaults: BTreeSet<ExportDefaultExprId>,
    top_level_callables: BTreeMap<CallableTemplateOrigin, PublicDeclarationOwnerV1>,
    top_level_properties: BTreeSet<PropertyDeclarationId>,
    pending: Vec<SourceWork>,
}

impl SourceRoots {
    fn callable(
        &mut self,
        export: &ExportHir,
        owner: ExportParameterOwner,
        roots: &mut Roots<'_>,
    ) -> Result<(), Error> {
        let Some((id, scope)) = identity::callable(export, owner)? else {
            return Ok(());
        };
        if scope.provider != export.cone || !insert(&mut self.callables, id, roots.meter)? {
            return Ok(());
        }
        match scope.owner {
            PublicDeclarationOwnerV1::Nominal(owner) => roots.require(owner, true)?,
            owner @ (PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension) => {
                work(roots.meter, self.top_level_callables.len())?;
                roots
                    .meter
                    .charge_collection_slots(1, &WirePath::root())
                    .map_err(resource)?;
                self.top_level_callables.insert(id, owner);
            }
        }
        push(&mut self.pending, SourceWork::Callable(id), roots.meter)
    }

    fn function(
        &mut self,
        export: &ExportHir,
        function: FunctionId,
        roots: &mut Roots<'_>,
    ) -> Result<(), Error> {
        match export
            .function_identities
            .get(function)
            .ok_or_else(|| invalid("default callable has no identity"))?
        {
            HirFunctionIdentity::Source(_) => {
                self.callable(export, ExportParameterOwner::Function(function), roots)
            }
            HirFunctionIdentity::PropertyAccessor(accessor) => {
                let identity = match *accessor {
                    HirPropertyAccessorFunction::Getter(id) => {
                        export.property_accessor_identities.get_getter(id)
                    }
                    HirPropertyAccessorFunction::Setter(id) => {
                        export.property_accessor_identities.get_setter(id)
                    }
                }
                .ok_or_else(|| invalid("default accessor has no logical property"))?;
                self.property(export, identity.property(), roots)
            }
            // Generated bodies are closed by their attached body metadata.
            HirFunctionIdentity::LexicalGenerated(_)
            | HirFunctionIdentity::Initialization { .. }
            | HirFunctionIdentity::DerivedEquality(_) => Ok(()),
        }
    }

    fn property(
        &mut self,
        export: &ExportHir,
        property: PropertyId,
        roots: &mut Roots<'_>,
    ) -> Result<(), Error> {
        let identity = export
            .property_identities
            .get(property)
            .ok_or_else(|| invalid("default property has no source identity"))?;
        let scope = identity::scope(identity.declaration())?;
        let id = identity.property_owner();
        if scope.provider != export.cone || !insert(&mut self.properties, id, roots.meter)? {
            return Ok(());
        }
        match scope.owner {
            PublicDeclarationOwnerV1::Nominal(owner) => roots.require(owner, true)?,
            PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension => {
                insert(&mut self.top_level_properties, id, roots.meter)?;
            }
        }
        push(
            &mut self.pending,
            SourceWork::Property(property),
            roots.meter,
        )
    }
}

fn insert<T: Copy + Ord>(
    set: &mut BTreeSet<T>,
    value: T,
    meter: &mut BudgetMeter,
) -> Result<bool, Error> {
    work(meter, set.len())?;
    if set.contains(&value) {
        return Ok(false);
    }
    meter
        .check_table_entries(set.len() as u64 + 1, &WirePath::root())
        .map_err(resource)?;
    meter
        .charge_collection_slots(1, &WirePath::root())
        .map_err(resource)?;
    set.insert(value);
    Ok(true)
}
