use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeIdentity, PersistentExactTypeId};

use super::CrossConeTypeSemanticsProductionError as Error;
use crate::*;

type FactProjection = (
    CanonicalExactTypeFactsV1,
    CanonicalPersistentIdsV1<PersistentExactTypeId>,
    Vec<TypeSectionDependencyFactV1>,
    BTreeMap<PersistentExactTypeId, ExactTypeFactShapeV1>,
);

pub(super) fn produce(
    imported_core: &SelectedImportedCoreSet<'_>,
    export: &ExportHir,
    local: &LocalConcreteHir,
    root_exacts: &BTreeSet<PersistentExactTypeId>,
    required_exacts: &BTreeSet<PersistentExactTypeId>,
) -> Result<FactProjection, Error> {
    let candidate = project(imported_core, export, local, root_exacts, required_exacts)?;
    let authority = project(imported_core, export, local, root_exacts, required_exacts)?;
    let facts = CanonicalExactTypeFactsV1::try_new(candidate.local_facts.into_values().collect())
        .map_err(|error| Error::InvalidTable {
        table: "exact-facts",
        reason: error.to_string(),
    })?;
    let local_exact_facts =
        CanonicalPersistentIdsV1::try_new(authority.local_facts.keys().copied().collect())
            .map_err(|error| Error::InvalidTable {
                table: "local-exact-fact inventory",
                reason: error.to_string(),
            })?;
    Ok((
        facts,
        local_exact_facts,
        authority.dependency_facts.into_values().collect(),
        authority.shapes,
    ))
}

fn project<'a>(
    imported_core: &'a SelectedImportedCoreSet<'_>,
    export: &'a ExportHir,
    local: &'a LocalConcreteHir,
    root_exacts: &'a BTreeSet<PersistentExactTypeId>,
    required_exacts: &BTreeSet<PersistentExactTypeId>,
) -> Result<FactProjector<'a>, Error> {
    let mut projector = FactProjector {
        imported_core,
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
    Ok(projector)
}

struct FactProjector<'a> {
    imported_core: &'a SelectedImportedCoreSet<'a>,
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
        if !force_local && self.is_core_leaf(ty) {
            if !self.imported_core.contains_hir_identity(exact) {
                return Err(Error::MissingLocalSupport(exact));
            }
            self.dependency_facts.insert(
                exact,
                TypeSectionDependencyFactV1 {
                    provider: ConeIdentity::CORE,
                    exact,
                },
            );
            return Ok(());
        }
        if self.is_generic_application(ty) {
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

    fn shape(&self, ty: concrete::TypeId) -> Result<ExactTypeFactShapeV1, Error> {
        use concrete::TypeKind;
        let shape = match &self.local.types[ty].kind {
            TypeKind::Unit => ExactTypeFactShapeV1::Unit,
            TypeKind::Integer(_) | TypeKind::Boolean => ExactTypeFactShapeV1::Scalar,
            TypeKind::String
            | TypeKind::Any
            | TypeKind::Class(_)
            | TypeKind::Interface(_)
            | TypeKind::Function(_) => ExactTypeFactShapeV1::Reference,
            TypeKind::Ptr(_) | TypeKind::FunPtr(_) => ExactTypeFactShapeV1::Pointer,
            TypeKind::Tuple(elements) => ExactTypeFactShapeV1::Tuple {
                elements: exacts(self.local, elements)?,
            },
            TypeKind::Struct(id) => {
                let structure = &self.local.structs[*id];
                if !structure.type_arguments.is_empty() {
                    return Err(Error::GenericOdrRequired(self.exact(ty)?));
                }
                match &structure.representation {
                    concrete::StructRepresentation::Declared { attributes, fields } => {
                        let fields = exacts(
                            self.local,
                            &fields.iter().map(|field| field.ty).collect::<Vec<_>>(),
                        )?;
                        if attributes.c_layout.is_some() {
                            ExactTypeFactShapeV1::CLayoutStruct { fields }
                        } else {
                            ExactTypeFactShapeV1::OrdinaryStruct { fields }
                        }
                    }
                    concrete::StructRepresentation::Intrinsic { application, .. } => {
                        match application {
                            concrete::IntrinsicTypeRepresentation::Integer(_)
                            | concrete::IntrinsicTypeRepresentation::Boolean => {
                                ExactTypeFactShapeV1::Scalar
                            }
                            concrete::IntrinsicTypeRepresentation::Ptr { .. }
                            | concrete::IntrinsicTypeRepresentation::FunPtr { .. } => {
                                ExactTypeFactShapeV1::Pointer
                            }
                            concrete::IntrinsicTypeRepresentation::String
                            | concrete::IntrinsicTypeRepresentation::Array { .. }
                            | concrete::IntrinsicTypeRepresentation::MutableArray { .. } => {
                                ExactTypeFactShapeV1::Reference
                            }
                        }
                    }
                }
            }
            TypeKind::Enum(id) => {
                let enumeration = &self.local.enums[*id];
                if !enumeration.type_arguments.is_empty() {
                    return Err(Error::GenericOdrRequired(self.exact(ty)?));
                }
                let exact = self.exact(ty)?;
                let owner = enumeration
                    .origin
                    .source()
                    .and_then(HirSourceNominalIdentity::concrete_id)
                    .ok_or(Error::MissingExactIdentity)?;
                let source_enum = self
                    .export
                    .enums
                    .iter()
                    .find_map(|(id, _)| {
                        (self.export.nominal_identities[id].concrete_type_id() == Some(owner))
                            .then_some(id)
                    })
                    .ok_or(Error::MissingConcreteType(exact))?;
                let mut variants = Vec::with_capacity(enumeration.variants.len());
                for (index, variant) in enumeration.variants.iter().enumerate() {
                    let index = u32::try_from(index).map_err(|_| Error::InvalidFact {
                        exact,
                        reason: "enum variant count exceeds typed identity index".into(),
                    })?;
                    let reference = EnumVariantRef::checked(&self.export.enums, source_enum, index)
                        .ok_or_else(|| Error::InvalidFact {
                            exact,
                            reason: format!("missing enum variant identity at index {index}"),
                        })?;
                    let variant_id = self.export.enum_member_identities[reference].id();
                    variants.push(ExactEnumVariantFactsV1 {
                        variant: variant_id,
                        fields: exacts(
                            self.local,
                            &variant
                                .fields
                                .iter()
                                .map(|field| field.ty)
                                .collect::<Vec<_>>(),
                        )?,
                        gc: if variant.gc_free {
                            ExactTypeGcV1::GcFree
                        } else {
                            ExactTypeGcV1::ContainsManagedReferences
                        },
                    });
                }
                ExactTypeFactShapeV1::Enum { variants }
            }
        };
        Ok(shape)
    }

    fn is_core_leaf(&self, ty: concrete::TypeId) -> bool {
        matches!(
            self.local.types[ty].kind,
            concrete::TypeKind::Unit
                | concrete::TypeKind::Integer(_)
                | concrete::TypeKind::Boolean
                | concrete::TypeKind::String
                | concrete::TypeKind::Any
        )
    }

    fn is_generic_application(&self, ty: concrete::TypeId) -> bool {
        match self.local.types[ty].kind {
            concrete::TypeKind::Struct(id) => !self.local.structs[id].type_arguments.is_empty(),
            concrete::TypeKind::Enum(id) => !self.local.enums[id].type_arguments.is_empty(),
            concrete::TypeKind::Class(id) => !self.local.classes[id].type_arguments.is_empty(),
            concrete::TypeKind::Interface(id) => {
                !self.local.interfaces[id].type_arguments.is_empty()
            }
            concrete::TypeKind::Unit
            | concrete::TypeKind::Integer(_)
            | concrete::TypeKind::Boolean
            | concrete::TypeKind::String
            | concrete::TypeKind::Any
            | concrete::TypeKind::Tuple(_)
            | concrete::TypeKind::Function(_)
            | concrete::TypeKind::Ptr(_)
            | concrete::TypeKind::FunPtr(_) => false,
        }
    }

    fn is_locally_owned(&self, ty: concrete::TypeId) -> Result<bool, Error> {
        let exact = self.exact(ty)?;
        Ok(self.root_exacts.contains(&exact)
            || matches!(
                self.local.types[ty].kind,
                concrete::TypeKind::Tuple(_)
                    | concrete::TypeKind::Function(_)
                    | concrete::TypeKind::Ptr(_)
                    | concrete::TypeKind::FunPtr(_)
            ))
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
