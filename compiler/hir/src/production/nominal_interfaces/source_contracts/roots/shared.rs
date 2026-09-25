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
    ) -> Result<Self, NominalInterfaceBuildError> {
        Self::collect(export).map_err(|error| match error {
            Error::SourceInventory(SourceInventoryError::Resource(error)) => {
                NominalInterfaceBuildError::Resource(error)
            }
            other => NominalInterfaceBuildError::Declarations(other.to_string()),
        })
    }

    fn collect(export: &ExportHir) -> Result<Self, Error> {
        let index = super::Index::new(export)?;
        let source_index = SourceIndex::new(export)?;
        let mut nominals = Roots {
            complete_children: true,
            required: BTreeMap::new(),
            pending: Vec::new(),
            field_types: BTreeSet::new(),
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
                if let Some(members) = source_index.members.get(&owner) {
                    for member in members {
                        match *member {
                            SourceWork::Callable(id) => {
                                let protocol = source_index.protocol(id)?;
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
                        source_index.protocol(id)?,
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
        scoop_wire::allocation::try_reserve(
            &mut required,
            nominals.required.len(),
            &WirePath::root(),
        )
        .map_err(resource)?;
        required.extend(nominals.required.into_keys());
        Ok(Self {
            nominals: CanonicalSourceNominalIdsV1::try_new(required)
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
        roots: &mut Roots,
    ) -> Result<(), Error> {
        let Some((id, scope)) = identity::callable(export, owner)? else {
            return Ok(());
        };
        if scope.provider != export.cone || !insert(&mut self.callables, id)? {
            return Ok(());
        }
        match scope.owner {
            PublicDeclarationOwnerV1::Nominal(owner) => roots.require(owner, true)?,
            owner @ (PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension) => {
                self.top_level_callables.insert(id, owner);
            }
        }
        push(&mut self.pending, SourceWork::Callable(id))
    }

    fn function(
        &mut self,
        export: &ExportHir,
        function: FunctionId,
        roots: &mut Roots,
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
        roots: &mut Roots,
    ) -> Result<(), Error> {
        let identity = export
            .property_identities
            .get(property)
            .ok_or_else(|| invalid("default property has no source identity"))?;
        let scope = identity::scope(identity.declaration())?;
        let id = identity.property_owner();
        if scope.provider != export.cone || !insert(&mut self.properties, id)? {
            return Ok(());
        }
        match scope.owner {
            PublicDeclarationOwnerV1::Nominal(owner) => roots.require(owner, true)?,
            PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension => {
                insert(&mut self.top_level_properties, id)?;
            }
        }
        push(&mut self.pending, SourceWork::Property(property))
    }
}

fn insert<T: Copy + Ord>(set: &mut BTreeSet<T>, value: T) -> Result<bool, Error> {
    if set.contains(&value) {
        return Ok(false);
    }

    set.insert(value);
    Ok(true)
}
