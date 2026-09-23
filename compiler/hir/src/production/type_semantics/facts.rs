use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeIdentity, PersistentExactTypeId};

use super::CrossConeTypeSemanticsProductionError as Error;
use crate::*;

mod ownership;
mod shapes;

#[derive(Clone, Copy)]
enum FactMode {
    Candidate,
    Source,
}

type FactSourceProjection = (
    CanonicalPersistentIdsV1<PersistentExactTypeId>,
    Vec<TypeSectionDependencyFactV1>,
    BTreeMap<PersistentExactTypeId, ExactTypeFactShapeV1>,
);

pub(super) fn candidate(
    export: &ExportHir,
    local: &LocalConcreteHir,
    root_exacts: &BTreeSet<PersistentExactTypeId>,
    required_exacts: &BTreeSet<PersistentExactTypeId>,
) -> Result<CanonicalExactTypeFactsV1, Error> {
    let candidate = project(
        FactMode::Candidate,
        export,
        local,
        root_exacts,
        required_exacts,
    )?;
    CanonicalExactTypeFactsV1::try_new(candidate.local_facts.into_values().collect()).map_err(
        |error| Error::InvalidTable {
            table: "exact-facts",
            reason: error.to_string(),
        },
    )
}

pub(super) fn source(
    export: &ExportHir,
    local: &LocalConcreteHir,
    root_exacts: &BTreeSet<PersistentExactTypeId>,
    required_exacts: &BTreeSet<PersistentExactTypeId>,
) -> Result<FactSourceProjection, Error> {
    source_projection(project(
        FactMode::Source,
        export,
        local,
        root_exacts,
        required_exacts,
    )?)
}

fn source_projection(authority: FactProjector<'_>) -> Result<FactSourceProjection, Error> {
    let local_exact_facts =
        CanonicalPersistentIdsV1::try_new(authority.local_facts.keys().copied().collect())
            .map_err(|error| Error::InvalidTable {
                table: "local-exact-fact inventory",
                reason: error.to_string(),
            })?;
    Ok((
        local_exact_facts,
        authority.dependency_facts.into_values().collect(),
        authority.shapes,
    ))
}

fn project<'a>(
    mode: FactMode,
    export: &'a ExportHir,
    local: &'a LocalConcreteHir,
    root_exacts: &'a BTreeSet<PersistentExactTypeId>,
    required_exacts: &BTreeSet<PersistentExactTypeId>,
) -> Result<FactProjector<'a>, Error> {
    let mut projector = FactProjector {
        mode,
        export,
        local,
        root_exacts,
        local_facts: BTreeMap::new(),
        shapes: BTreeMap::new(),
        dependency_facts: BTreeMap::new(),
        active: BTreeSet::new(),
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
    mode: FactMode,
    export: &'a ExportHir,
    local: &'a LocalConcreteHir,
    root_exacts: &'a BTreeSet<PersistentExactTypeId>,
    local_facts: BTreeMap<PersistentExactTypeId, ExactTypeFactsV1>,
    shapes: BTreeMap<PersistentExactTypeId, ExactTypeFactShapeV1>,
    dependency_facts: BTreeMap<PersistentExactTypeId, TypeSectionDependencyFactV1>,
    active: BTreeSet<PersistentExactTypeId>,
}

impl FactProjector<'_> {
    fn visit_exact(
        &mut self,
        exact: PersistentExactTypeId,
        force_local: bool,
    ) -> Result<(), Error> {
        if self.local_facts.contains_key(&exact) || self.dependency_facts.contains_key(&exact) {
            return Ok(());
        }
        let ty = self
            .local
            .exact_type_identities
            .type_for_identity(exact)
            .ok_or(Error::MissingConcreteType(exact))?;
        if !force_local && let Some(provider) = self.dependency_provider(ty) {
            self.dependency_facts
                .insert(exact, TypeSectionDependencyFactV1 { provider, exact });
            return Ok(());
        }
        if matches!(self.mode, FactMode::Candidate) && self.is_generic_application(ty) {
            return Err(Error::GenericOdrRequired(exact));
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
                let zero = fields.iter().all(|field| self.is_zero_sized(*field));
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
        self.shapes.insert(exact, shape);
        self.local_facts.insert(exact, fact);
        Ok(())
    }

    fn exact(&self, ty: concrete::TypeId) -> Result<PersistentExactTypeId, Error> {
        self.local
            .exact_type_identities
            .get(ty)
            .map(|record| record.id())
            .ok_or(Error::MissingExactIdentity)
    }

    fn is_zero_sized(&self, exact: PersistentExactTypeId) -> bool {
        self.local_facts.get(&exact).is_some_and(|fact| {
            fact.kind()
                == ExactTypeKindV1::Value {
                    zst: ZstStatus::ZeroSized,
                }
        }) || self
            .local
            .exact_type_identities
            .type_for_identity(exact)
            .is_some_and(|ty| matches!(self.local.types[ty].kind, concrete::TypeKind::Unit))
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
                .ok_or(Error::MissingExactIdentity)
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
