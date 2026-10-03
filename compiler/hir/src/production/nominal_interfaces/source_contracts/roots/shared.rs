//! Rooted source dependencies from sealed HIR, before any interface is projected.

use super::*;
use scoop_identity::{CallableTemplateOrigin, ConeIdentity, DefinitionOwnerAtom};

mod defaults;
mod identity;
mod index;
mod protocols;
mod templates;
use index::SourceIndex;

#[derive(Debug)]
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

    pub(super) fn collect(export: &ExportHir) -> Result<Self, Error> {
        let mut collection = SourceCollection::new(export)?;
        collection.expand()?;
        collection.snapshot()
    }
}

struct SourceCollection<'a> {
    export: &'a ExportHir,
    index: super::Index,
    source_index: SourceIndex<'a>,
    nominals: Roots,
    sources: SourceRoots,
}

impl<'a> SourceCollection<'a> {
    fn new(export: &'a ExportHir) -> Result<Self, Error> {
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
            let source = local
                .identity(export)
                .and_then(HirNominalIdentity::source)
                .ok_or_else(|| invalid("public nominal has no source identity"))?;
            if source.declaration().origin() == export.cone {
                nominals.require(source_nominal_id(source), true)?;
            }
        }
        for function in &export.public_surface.functions {
            sources.function(export, *function, &mut nominals)?;
        }
        if let crate::CoreProtocols::Defined(protocols) = &export.core_protocols {
            sources.function(
                export,
                protocols.exceptions.initialization_cycle_thrower,
                &mut nominals,
            )?;
        }
        for property in &export.public_surface.properties {
            sources.property(export, *property, &mut nominals)?;
        }
        Ok(Self {
            export,
            index,
            source_index,
            nominals,
            sources,
        })
    }

    fn expand(&mut self) -> Result<(), Error> {
        let Self {
            export,
            index,
            source_index,
            nominals,
            sources,
        } = self;
        loop {
            while let Some(owner) = nominals.expand_next(export, index)? {
                if let Some(members) = source_index.members.get(&owner) {
                    for member in members {
                        match *member {
                            SourceWork::Callable(id) => {
                                let protocol = source_index.protocol(id)?;
                                sources.callable(export, protocol.owner, nominals)?;
                            }
                            SourceWork::Property(id) => {
                                sources.property(export, id, nominals)?;
                            }
                        }
                    }
                }
            }
            while let Some(next) = sources.pending.pop() {
                match next {
                    SourceWork::Callable(id) => {
                        sources.protocol(export, source_index.protocol(id)?, index, nominals)?
                    }
                    SourceWork::Property(id) => {
                        sources.property_types(export, id, index, nominals)?;
                    }
                }
            }
            if nominals.pending.is_empty() {
                break;
            }
        }
        Ok(())
    }

    fn has_pending(&self) -> bool {
        !self.nominals.pending.is_empty() || !self.sources.pending.is_empty()
    }

    fn snapshot(&self) -> Result<SharedSourceRoots, Error> {
        Ok(SharedSourceRoots {
            nominals: CanonicalSourceNominalIdsV1::try_new(
                self.nominals.required.keys().copied().collect(),
            )
            .map_err(Error::SourceInventory)?,
            top_level_callables: self.sources.top_level_callables.clone(),
            top_level_properties: self.sources.top_level_properties.clone(),
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
