//! Borrowed semantic dependencies from the shared nominal and callable tables.

use super::*;
use crate::{
    CallableDeclarationRecordV1, DeclaredVisibilityV1, EnumSourceFieldV1, NominalSourceFieldV1,
    NominalSourceShapeV1, PublicDeclarationOwnerV1,
};
use scoop_identity::{CallableTemplateOrigin, SignatureTypeKey};

/// A dependency position in the original declaration, not a transport record
/// or a machine-use permit. Repeated types retain their distinct source sites.
#[derive(Clone, Copy, Debug)]
pub enum NominalMaterializationRequirementV1<'a> {
    Field {
        owner: PersistentTypeId,
        field: &'a NominalSourceFieldV1,
    },
    EnumVariantField {
        owner: PersistentTypeId,
        field: &'a EnumSourceFieldV1,
    },
    Inheritance {
        owner: PersistentTypeId,
        parent: &'a SignatureTypeKey,
    },
    Constructor {
        owner: PersistentTypeId,
        callable: &'a CallableDeclarationRecordV1,
    },
    Slot {
        owner: PersistentTypeId,
        callable: &'a CallableDeclarationRecordV1,
    },
}

impl NominalMaterializationRequirementV1<'_> {
    pub const fn owner(self) -> PersistentTypeId {
        match self {
            Self::Field { owner, .. }
            | Self::EnumVariantField { owner, .. }
            | Self::Inheritance { owner, .. }
            | Self::Constructor { owner, .. }
            | Self::Slot { owner, .. } => owner,
        }
    }
}

impl CanonicalNominalInterfacesV1 {
    /// Visits all parameter-free nominal machine requirements, including
    /// private storage.
    pub fn visit_materialization_requirements<'a, E>(
        &'a self,
        callables: &'a CanonicalCallableInterfacesV1,

        mut visit: impl FnMut(NominalMaterializationRequirementV1<'a>) -> Result<(), E>,
    ) -> Result<(), E>
    where
        E: From<NominalMaterializationClosureError>,
    {
        use NominalMaterializationRequirementV1 as Requirement;

        let mut dispatched = BTreeSet::new();
        for nominal in self.all_records() {
            let SourceNominalId::Concrete(owner) = nominal.declaration() else {
                continue;
            };
            let selections = nominal
                .declaration_details()
                .dispatch_selections()
                .records();

            for selection in selections {
                if let Some(target) = selection.callable_target()
                    && !dispatched.contains(&target)
                {
                    dispatched.insert(target);
                }
            }
            let fields = nominal.source_shape().declared_fields();

            for field in fields {
                visit(Requirement::Field { owner, field })?;
            }
            if let NominalSourceShapeV1::Enum(shape) = nominal.source_shape() {
                for variant in shape.variants() {
                    for field in variant.fields() {
                        visit(Requirement::EnumVariantField { owner, field })?;
                    }
                }
            }

            for parent in nominal.exact_supertypes().values() {
                visit(Requirement::Inheritance { owner, parent })?;
            }
        }
        for callable in callables.all_declarations() {
            let PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(owner)) =
                callable.owner()
            else {
                continue;
            };
            let constructor = matches!(
                callable.declaration(),
                CallableTemplateOrigin::Constructor(_)
                    | CallableTemplateOrigin::VariantConstructor(_)
            ) && matches!(
                callable.declared_visibility(),
                DeclaredVisibilityV1::Public | DeclaredVisibilityV1::Protected
            );

            if !constructor
                && callable.slot_relations().is_empty()
                && !dispatched.contains(&callable.declaration())
            {
                continue;
            }

            if self.declaration(SourceNominalId::Concrete(owner)).is_none() {
                return Err(NominalMaterializationClosureError::MissingNominal(owner).into());
            }
            let requirement = if constructor {
                Requirement::Constructor { owner, callable }
            } else {
                Requirement::Slot { owner, callable }
            };
            visit(requirement)?;
        }
        Ok(())
    }
}
