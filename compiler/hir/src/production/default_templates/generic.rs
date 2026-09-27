//! Portable callable implementations selected from the shared source surface.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use scoop_identity::{CallableTemplateOrigin, DeclarationScope, DefinitionOwnerAtom};
use scoop_wire::WireError;

use super::entities::DefaultEntityProjector;
use crate::{
    CanonicalExportGenericCallableBodiesV1, DefaultCallableDeclarationV1, DependencyHirOutput,
    ExportHir, FunctionId, FunctionKind, HirFunctionIdentity, HirPropertyAccessorFunction,
    SelectedImportedDependencySet, SourceNominalId,
};

mod captures;
mod errors;
mod projection;
pub use errors::GenericTemplateProductionError;
pub(in crate::production::default_templates) use projection::binder_uses;

use crate::production::nominal_interfaces::SharedSourceRoots;

impl CanonicalExportGenericCallableBodiesV1 {
    pub fn from_export_hir(export: &ExportHir) -> Result<Self, GenericTemplateProductionError> {
        SharedSourceRoots::with_callable_bodies(export, None).map(|(_, bodies, _)| bodies)
    }

    pub fn from_dependency_hir(
        output: &DependencyHirOutput,
    ) -> Result<Self, GenericTemplateProductionError> {
        Ok(output.output().export.shared_source().bodies.clone())
    }
}

pub(in crate::production) struct GenericBodyProducer<'a> {
    entities: DefaultEntityProjector<'a>,
    index: BTreeMap<DefaultCallableDeclarationV1, FunctionId>,
    pending: VecDeque<FunctionId>,
    scheduled: BTreeSet<FunctionId>,
}

impl<'a> GenericBodyProducer<'a> {
    pub(in crate::production) fn new(
        export: &'a ExportHir,
        imported: Option<&'a SelectedImportedDependencySet>,
    ) -> Result<Self, GenericTemplateProductionError> {
        let entities = DefaultEntityProjector::new(export, imported);
        let mut index = BTreeMap::new();
        for (function, declaration) in export.functions.iter() {
            if matches!(declaration.kind, FunctionKind::DerivedEquality) {
                continue;
            }
            let owner = entities
                .callable_declaration(function)
                .map_err(GenericTemplateProductionError::Entity)?;
            index.insert(owner, function);
        }
        Ok(Self {
            entities,
            index,
            pending: VecDeque::new(),
            scheduled: BTreeSet::new(),
        })
    }

    pub(in crate::production) fn include_roots(&mut self, roots: &SharedSourceRoots) {
        for &function in self.index.values() {
            let export = self.entities.export();
            if export.functions[function].type_param_count() != 0
                && is_shared_root(export, function, roots)
                && self.scheduled.insert(function)
            {
                self.pending.push_back(function);
            }
        }
    }

    pub(in crate::production) fn next_body(
        &mut self,
    ) -> Result<Option<crate::ExportGenericCallableBodyV1>, GenericTemplateProductionError> {
        while let Some(function) = self.pending.pop_front() {
            if let Some(body) = projection::project(&self.entities, function)? {
                return Ok(Some(body));
            }
        }
        Ok(None)
    }

    pub(in crate::production) fn require(
        &mut self,
        owner: DefaultCallableDeclarationV1,
    ) -> Option<FunctionId> {
        let &function = self.index.get(&owner)?;
        let export = self.entities.export();
        let lexical = match &export.function_identities[function] {
            HirFunctionIdentity::Source(source) => matches!(
                source.declaration().scope(),
                DeclarationScope::LexicalScoped { .. }
            ),
            HirFunctionIdentity::LexicalGenerated(_) => true,
            HirFunctionIdentity::PropertyAccessor(_)
            | HirFunctionIdentity::Initialization { .. }
            | HirFunctionIdentity::DerivedEquality(_) => false,
        };
        if (lexical || export.functions[function].type_param_count() != 0)
            && self.scheduled.insert(function)
        {
            self.pending.push_back(function);
        }
        Some(function)
    }
}

fn is_shared_root(export: &ExportHir, function: FunctionId, roots: &SharedSourceRoots) -> bool {
    match &export.function_identities[function] {
        HirFunctionIdentity::Source(source) => {
            let key = source.declaration();
            if key.origin() != export.cone
                || matches!(key.scope(), DeclarationScope::LexicalScoped { .. })
            {
                return false;
            }
            match key.owners().owners().last() {
                Some(DefinitionOwnerAtom::Type(owner)) => roots
                    .nominals
                    .values()
                    .contains(&SourceNominalId::Concrete(*owner)),
                Some(DefinitionOwnerAtom::GenericType(owner)) => roots
                    .nominals
                    .values()
                    .contains(&SourceNominalId::GenericTemplate(*owner)),
                None => {
                    let owner = match source {
                        crate::HirSourceFunctionIdentity::Plain(record) => {
                            CallableTemplateOrigin::Function(record.id())
                        }
                        crate::HirSourceFunctionIdentity::Generic(record) => {
                            CallableTemplateOrigin::GenericFunction(record.id())
                        }
                    };
                    roots.top_level_callables.contains_key(&owner)
                }
                Some(_) => false,
            }
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
            .expect("the entity projector already checked this immutable accessor identity");
            let property = &export.property_identities[identity.property()];
            if property.declaration().origin() != export.cone {
                return false;
            }
            match property.declaration().owners().owners().last() {
                Some(DefinitionOwnerAtom::Type(owner)) => roots
                    .nominals
                    .values()
                    .contains(&SourceNominalId::Concrete(*owner)),
                Some(DefinitionOwnerAtom::GenericType(owner)) => roots
                    .nominals
                    .values()
                    .contains(&SourceNominalId::GenericTemplate(*owner)),
                None => roots
                    .top_level_properties
                    .contains(&property.property_owner()),
                Some(_) => false,
            }
        }
        HirFunctionIdentity::LexicalGenerated(_)
        | HirFunctionIdentity::Initialization { .. }
        | HirFunctionIdentity::DerivedEquality(_) => false,
    }
}

impl From<WireError> for GenericTemplateProductionError {
    fn from(error: WireError) -> Self {
        Self::Wire(error)
    }
}
