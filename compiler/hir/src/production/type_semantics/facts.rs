use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeIdentity, PersistentExactTypeId};

use super::CrossConeTypeSemanticsProductionError as Error;
use crate::*;

mod ownership;
mod shapes;

pub(super) fn produce(
    export: &ExportHir,
    local: &LocalConcreteHir,
    root_exacts: &BTreeSet<PersistentExactTypeId>,
    required_exacts: &BTreeSet<PersistentExactTypeId>,
) -> Result<CanonicalExactTypeFactsV1, Error> {
    let candidate = project(export, local, root_exacts, required_exacts)?;
    CanonicalExactTypeFactsV1::try_new(candidate.local_facts.into_values().collect()).map_err(
        |error| Error::InvalidTable {
            table: "exact-facts",
            reason: error.to_string(),
        },
    )
}

fn project<'a>(
    export: &'a ExportHir,
    local: &'a LocalConcreteHir,
    root_exacts: &'a BTreeSet<PersistentExactTypeId>,
    required_exacts: &BTreeSet<PersistentExactTypeId>,
) -> Result<FactProjector<'a>, Error> {
    let mut projector = FactProjector {
        export,
        local,
        root_exacts,
        local_facts: BTreeMap::new(),
        dependency_types: BTreeSet::new(),
        active: BTreeSet::new(),
        zero_sized_values: BTreeMap::new(),
    };
    for exact in root_exacts {
        projector.visit_exact(*exact, true)?;
    }
    for exact in required_exacts {
        projector.visit_exact(*exact, root_exacts.contains(exact))?;
    }
    for (ty, _) in local.types.iter() {
        if projector.is_local_builtin(ty) {
            projector.visit_exact(projector.exact(ty)?, false)?;
        }
    }
    Ok(projector)
}

struct FactProjector<'a> {
    export: &'a ExportHir,
    local: &'a LocalConcreteHir,
    root_exacts: &'a BTreeSet<PersistentExactTypeId>,
    local_facts: BTreeMap<PersistentExactTypeId, ExactTypeFactsV1>,
    dependency_types: BTreeSet<PersistentExactTypeId>,
    active: BTreeSet<PersistentExactTypeId>,
    zero_sized_values: BTreeMap<PersistentExactTypeId, bool>,
}

impl FactProjector<'_> {
    fn visit_exact(
        &mut self,
        exact: PersistentExactTypeId,
        force_local: bool,
    ) -> Result<(), Error> {
        if self.local_facts.contains_key(&exact) || self.dependency_types.contains(&exact) {
            return Ok(());
        }
        let ty = self
            .local
            .exact_type_identities
            .type_for_identity(exact)
            .ok_or(Error::MissingConcreteType(exact))?;
        if !force_local && self.dependency_provider(ty).is_some() {
            self.dependency_types.insert(exact);
            return Ok(());
        }
        if !force_local && !self.is_locally_owned(ty)? {
            return Err(Error::MissingLocalSupport(exact));
        }
        if !self.active.insert(exact) {
            return Err(Error::InvalidFact {
                exact,
                reason: "recursive by-value type".into(),
            });
        }
        let shape = self.shape(ty)?;
        for child in shape_children(&shape) {
            self.visit_exact(child, self.root_exacts.contains(&child))?;
        }
        let kind = match &shape {
            ExactTypeFactShapeV1::Unit => ExactTypeKindV1::Value {
                zst: ZstStatus::ZeroSized,
            },
            ExactTypeFactShapeV1::Scalar
            | ExactTypeFactShapeV1::Pointer
            | ExactTypeFactShapeV1::CLayoutStruct { .. }
            | ExactTypeFactShapeV1::Enum { .. } => ExactTypeKindV1::Value {
                zst: ZstStatus::NonZero,
            },
            ExactTypeFactShapeV1::Reference => ExactTypeKindV1::Reference,
            ExactTypeFactShapeV1::OrdinaryStruct { fields }
            | ExactTypeFactShapeV1::Tuple { elements: fields } => {
                let mut zero = true;
                for field in fields {
                    zero &= self.is_zero_sized(*field)?;
                }
                ExactTypeKindV1::Value {
                    zst: if zero {
                        ZstStatus::ZeroSized
                    } else {
                        ZstStatus::NonZero
                    },
                }
            }
        };
        let gc = if matches!(kind, ExactTypeKindV1::Reference) {
            ExactTypeGcV1::ContainsManagedReferences
        } else if self.local.types[ty].gc_free {
            ExactTypeGcV1::GcFree
        } else {
            ExactTypeGcV1::ContainsManagedReferences
        };
        let fact =
            ExactTypeFactsV1::try_new(exact, kind, gc).map_err(|error| Error::InvalidFact {
                exact,
                reason: error.to_string(),
            })?;
        self.active.remove(&exact);
        self.local_facts.insert(exact, fact);
        Ok(())
    }

    fn exact(&self, ty: concrete::TypeId) -> Result<PersistentExactTypeId, Error> {
        self.local
            .exact_type_identities
            .get(ty)
            .map(|record| record.id())
            .ok_or(Error::MissingExactIdentity {
                context: "concrete fact type",
            })
    }

    fn is_zero_sized(&mut self, exact: PersistentExactTypeId) -> Result<bool, Error> {
        if let Some(fact) = self.local_facts.get(&exact) {
            return Ok(fact.kind()
                == ExactTypeKindV1::Value {
                    zst: ZstStatus::ZeroSized,
                });
        }
        if let Some(zero) = self.zero_sized_values.get(&exact) {
            return Ok(*zero);
        }
        let ty = self
            .local
            .exact_type_identities
            .type_for_identity(exact)
            .ok_or(Error::MissingConcreteType(exact))?;
        if !self.active.insert(exact) {
            return Err(Error::InvalidFact {
                exact,
                reason: "recursive by-value type".into(),
            });
        }
        let zero = match &self.local.types[ty].kind {
            concrete::TypeKind::Unit => true,
            concrete::TypeKind::Tuple(elements) => {
                let mut zero = true;
                for element in elements {
                    zero &= self.is_zero_sized(self.exact(*element)?)?;
                }
                zero
            }
            concrete::TypeKind::Struct(id) => match &self.local.structs[*id].representation {
                concrete::StructRepresentation::Declared {
                    attributes, fields, ..
                } if attributes.c_layout.is_none() => {
                    let mut zero = true;
                    for field in fields {
                        zero &= self.is_zero_sized(self.exact(field.ty)?)?;
                    }
                    zero
                }
                _ => false,
            },
            _ => false,
        };
        self.active.remove(&exact);
        self.zero_sized_values.insert(exact, zero);
        Ok(zero)
    }
}

fn exacts(
    local: &LocalConcreteHir,
    types: &[concrete::TypeId],
) -> Result<Vec<PersistentExactTypeId>, Error> {
    types
        .iter()
        .map(|ty| {
            local
                .exact_type_identities
                .get(*ty)
                .map(|record| record.id())
                .ok_or(Error::MissingExactIdentity {
                    context: "source fact type",
                })
        })
        .collect()
}

fn shape_children(shape: &ExactTypeFactShapeV1) -> Vec<PersistentExactTypeId> {
    match shape {
        ExactTypeFactShapeV1::OrdinaryStruct { fields }
        | ExactTypeFactShapeV1::CLayoutStruct { fields }
        | ExactTypeFactShapeV1::Tuple { elements: fields } => fields.clone(),
        ExactTypeFactShapeV1::Enum { variants } => variants
            .iter()
            .flat_map(|variant| variant.fields.iter().copied())
            .collect(),
        ExactTypeFactShapeV1::Unit
        | ExactTypeFactShapeV1::Scalar
        | ExactTypeFactShapeV1::Pointer
        | ExactTypeFactShapeV1::Reference => Vec::new(),
    }
}
