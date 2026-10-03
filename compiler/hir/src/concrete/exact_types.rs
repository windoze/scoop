use std::collections::{HashMap, HashSet};
use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CallingConvention, CborIdentityRecord, CoreBuiltinNominal, Effect, ExactTypeKey, NonEmptyVec,
    OdrGroupId, PersistentExactTypeId, SpecializationKey,
};

use super::{
    ClassDef, ClassId, EnumDef, EnumId, FunctionType, InterfaceDef, InterfaceId, IntrinsicTypeCore,
    ObjectDecl, StructDef, StructId, Type, TypeId, TypeKind,
};
use crate::HirNominalIdentity;

mod error;
mod origins;
pub use error::{ExactTypeIdentityError, ExactTypeRelation};

#[cfg(test)]
mod tests;

type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;
pub type NominalSpecializationRecord = CborIdentityRecord<OdrGroupId, SpecializationKey>;

#[derive(Clone, Copy)]
pub struct ExactTypeIdentityInputs<'a> {
    pub types: &'a Arena<Type>,
    pub function_types: &'a Arena<FunctionType>,
    pub structs: &'a Arena<StructDef>,
    pub enums: &'a Arena<EnumDef>,
    pub classes: &'a Arena<ClassDef>,
    pub interfaces: &'a Arena<InterfaceDef>,
    pub objects: &'a Arena<ObjectDecl>,
    pub core_types: ConcreteCoreTypeIdentityAuthority<'a>,
}

/// Origin-refined exact-type authority for fundamental types in LocalConcrete
/// HIR. Imported owners never require a synthetic local nominal definition.
#[derive(Clone, Copy)]
pub enum ConcreteCoreTypeIdentityAuthority<'a> {
    Defined(&'a IntrinsicTypeCore),
    Imported(&'a crate::ImportedCoreFundamentalTypeProtocol),
}

/// Total persistent exact-type relation for LocalConcrete HIR.
#[derive(Clone, Debug)]
pub struct ExactTypeIdentities {
    identities: Vec<ExactTypeRecord>,
    types_by_identity: HashMap<PersistentExactTypeId, TypeId>,
    nominal_specializations: Vec<Option<usize>>,
    nominal_specialization_records: Vec<NominalSpecializationRecord>,
}

impl ExactTypeIdentities {
    pub fn from_types(inputs: ExactTypeIdentityInputs<'_>) -> Result<Self, ExactTypeIdentityError> {
        ExactTypeIdentityBuilder::new(inputs).build()
    }

    pub fn get(&self, id: TypeId) -> Option<&ExactTypeRecord> {
        self.identities.get(local_index(id))
    }

    pub fn len(&self) -> usize {
        self.identities.len()
    }

    pub fn is_empty(&self) -> bool {
        self.identities.is_empty()
    }

    pub fn type_for_identity(&self, identity: PersistentExactTypeId) -> Option<TypeId> {
        self.types_by_identity.get(&identity).copied()
    }

    pub fn nominal_specialization(&self, ty: TypeId) -> Option<&NominalSpecializationRecord> {
        self.nominal_specializations
            .get(local_index(ty))
            .and_then(|position| *position)
            .map(|position| &self.nominal_specialization_records[position])
    }

    pub fn nominal_specialization_records(&self) -> &[NominalSpecializationRecord] {
        &self.nominal_specialization_records
    }
}

impl Index<TypeId> for ExactTypeIdentities {
    type Output = ExactTypeRecord;

    fn index(&self, id: TypeId) -> &Self::Output {
        &self.identities[local_index(id)]
    }
}

struct ExactTypeIdentityBuilder<'a> {
    inputs: ExactTypeIdentityInputs<'a>,
    identities: Vec<Option<ExactTypeRecord>>,
    visiting: Vec<bool>,
    exact_ids: HashSet<PersistentExactTypeId>,
    nominal_specialization_ids: HashSet<OdrGroupId>,
    nominal_specializations: Vec<Option<OdrGroupId>>,
    nominal_specialization_records: Vec<NominalSpecializationRecord>,
    object_by_backing_class: HashMap<ClassId, HirNominalIdentity>,
}

impl<'a> ExactTypeIdentityBuilder<'a> {
    fn new(inputs: ExactTypeIdentityInputs<'a>) -> Self {
        Self {
            identities: vec![None; inputs.types.len()],
            visiting: vec![false; inputs.types.len()],
            exact_ids: HashSet::new(),
            nominal_specialization_ids: HashSet::new(),
            nominal_specializations: vec![None; inputs.types.len()],
            nominal_specialization_records: Vec::new(),
            object_by_backing_class: HashMap::new(),
            inputs,
        }
    }

    fn build(mut self) -> Result<ExactTypeIdentities, ExactTypeIdentityError> {
        for (_, declaration) in self.inputs.objects.iter() {
            self.require_class(
                None,
                ExactTypeRelation::ObjectBackingClass,
                declaration.backing_class,
            )?;
            if self
                .object_by_backing_class
                .insert(declaration.backing_class, declaration.origin.clone())
                .is_some()
            {
                return Err(ExactTypeIdentityError::DuplicateObjectBackingClass {
                    class: raw_index(declaration.backing_class),
                });
            }
        }

        let ids = self
            .inputs
            .types
            .iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        for id in ids {
            self.resolve(id)?;
        }
        let identities = self
            .identities
            .into_iter()
            .enumerate()
            .map(|(index, identity)| {
                identity.ok_or(ExactTypeIdentityError::MissingIdentity { ty: index as u32 })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let types_by_identity = identities
            .iter()
            .enumerate()
            .map(|(index, identity)| {
                (
                    identity.id(),
                    TypeId::from_raw(u32::try_from(index).expect("type ids fit in u32").into()),
                )
            })
            .collect();
        self.nominal_specialization_records
            .sort_by_key(CborIdentityRecord::id);
        let specialization_positions = self
            .nominal_specialization_records
            .iter()
            .enumerate()
            .map(|(position, record)| (record.id(), position))
            .collect::<HashMap<_, _>>();
        let nominal_specializations = self
            .nominal_specializations
            .into_iter()
            .map(|group| group.map(|group| specialization_positions[&group]))
            .collect();
        Ok(ExactTypeIdentities {
            identities,
            types_by_identity,
            nominal_specializations,
            nominal_specialization_records: self.nominal_specialization_records,
        })
    }

    fn resolve(&mut self, ty: TypeId) -> Result<ExactTypeRecord, ExactTypeIdentityError> {
        let index = self.require_type(None, ExactTypeRelation::ChildType, ty)?;
        if let Some(identity) = &self.identities[index] {
            return Ok(identity.clone());
        }
        if std::mem::replace(&mut self.visiting[index], true) {
            return Err(ExactTypeIdentityError::Cycle { ty: raw_index(ty) });
        }

        let kind = self.inputs.types[ty].kind.clone();
        let key = match kind {
            TypeKind::Unit => {
                ExactTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id())
            }
            TypeKind::Integer(kind) => match self.inputs.core_types {
                ConcreteCoreTypeIdentityAuthority::Defined(core) => {
                    let owner = core.integers.owner(kind);
                    self.require_struct(Some(ty), ExactTypeRelation::IntrinsicNominalOwner, owner)?;
                    let declaration = &self.inputs.structs[owner];
                    self.require_canonical_nominal(
                        ty,
                        declaration.canonical_type,
                        ExactTypeRelation::IntrinsicNominalOwner,
                    )?;
                    let identity = declaration.origin.clone();
                    self.nominal_key(ty, identity, &[])?
                }
                ConcreteCoreTypeIdentityAuthority::Imported(core) => {
                    ExactTypeKey::Nominal(core.integer(kind).persistent())
                }
            },
            TypeKind::Boolean => match self.inputs.core_types {
                ConcreteCoreTypeIdentityAuthority::Defined(core) => {
                    let owner = core.boolean;
                    self.require_struct(Some(ty), ExactTypeRelation::IntrinsicNominalOwner, owner)?;
                    let declaration = &self.inputs.structs[owner];
                    self.require_canonical_nominal(
                        ty,
                        declaration.canonical_type,
                        ExactTypeRelation::IntrinsicNominalOwner,
                    )?;
                    let identity = declaration.origin.clone();
                    self.nominal_key(ty, identity, &[])?
                }
                ConcreteCoreTypeIdentityAuthority::Imported(core) => {
                    ExactTypeKey::Nominal(core.boolean().persistent())
                }
            },
            TypeKind::String => match self.inputs.core_types {
                ConcreteCoreTypeIdentityAuthority::Defined(core) => {
                    let owner = core.string;
                    self.require_class(Some(ty), ExactTypeRelation::IntrinsicNominalOwner, owner)?;
                    let declaration = &self.inputs.classes[owner];
                    self.require_canonical_nominal(
                        ty,
                        declaration.canonical_type,
                        ExactTypeRelation::IntrinsicNominalOwner,
                    )?;
                    let identity = declaration.origin.clone();
                    self.nominal_key(ty, identity, &[])?
                }
                ConcreteCoreTypeIdentityAuthority::Imported(core) => {
                    ExactTypeKey::Nominal(core.string().persistent())
                }
            },
            TypeKind::Any => ExactTypeKey::Nominal(CoreBuiltinNominal::Any.identity_record().id()),
            TypeKind::Struct(id) => {
                self.require_struct(Some(ty), ExactTypeRelation::Struct, id)?;
                let declaration = &self.inputs.structs[id];
                self.require_canonical_nominal(
                    ty,
                    declaration.canonical_type,
                    ExactTypeRelation::Struct,
                )?;
                let identity = declaration.origin.clone();
                let arguments = declaration.type_arguments.clone();
                self.nominal_key(ty, identity, &arguments)?
            }
            TypeKind::Enum(id) => {
                self.require_enum(ty, id)?;
                let declaration = &self.inputs.enums[id];
                self.require_canonical_nominal(
                    ty,
                    declaration.canonical_type,
                    ExactTypeRelation::Enum,
                )?;
                let identity = declaration.origin.clone();
                let arguments = declaration.type_arguments.clone();
                self.nominal_key(ty, identity, &arguments)?
            }
            TypeKind::Class(id) => {
                self.require_class(Some(ty), ExactTypeRelation::Class, id)?;
                let declaration = &self.inputs.classes[id];
                self.require_canonical_nominal(
                    ty,
                    declaration.canonical_type,
                    ExactTypeRelation::Class,
                )?;
                if let Some(identity) = self.object_by_backing_class.get(&id).cloned() {
                    let arguments = declaration.type_arguments.clone();
                    self.nominal_key(ty, identity, &arguments)?
                } else {
                    let identity = declaration.origin.clone();
                    let arguments = declaration.type_arguments.clone();
                    self.nominal_key(ty, identity, &arguments)?
                }
            }
            TypeKind::Interface(id) => {
                self.require_interface(ty, id)?;
                let declaration = &self.inputs.interfaces[id];
                self.require_canonical_nominal(
                    ty,
                    declaration.canonical_type,
                    ExactTypeRelation::Interface,
                )?;
                let identity = declaration.origin.clone();
                let arguments = declaration.type_arguments.clone();
                self.nominal_key(ty, identity, &arguments)?
            }
            TypeKind::Tuple(elements) => {
                let elements = NonEmptyVec::new(self.resolve_children(ty, &elements)?)
                    .map_err(|_| ExactTypeIdentityError::EmptyTuple { ty: raw_index(ty) })?;
                ExactTypeKey::Tuple(elements)
            }
            TypeKind::Function(function) => self.function_key(ty, function, false)?,
            TypeKind::Ptr(pointee) => {
                self.require_type(Some(ty), ExactTypeRelation::ChildType, pointee)?;
                ExactTypeKey::RawPointer(self.resolve(pointee)?.id())
            }
            TypeKind::FunPtr(function) => self.function_key(ty, function, true)?,
        };
        let identity = CborIdentityRecord::from_key(key).map_err(|error| {
            ExactTypeIdentityError::InvalidIdentity {
                ty: raw_index(ty),
                error,
            }
        })?;
        if !self.exact_ids.insert(identity.id()) {
            return Err(ExactTypeIdentityError::DuplicateIdentity { ty: raw_index(ty) });
        }
        if let ExactTypeKey::NominalApplication { origin, arguments } = identity.key() {
            let specialization = CborIdentityRecord::from_key(SpecializationKey::Nominal {
                origin: *origin,
                arguments: arguments.clone(),
            })
            .map_err(|error| {
                ExactTypeIdentityError::InvalidNominalSpecialization {
                    ty: raw_index(ty),
                    error,
                }
            })?;
            if !self.nominal_specialization_ids.insert(specialization.id()) {
                return Err(ExactTypeIdentityError::DuplicateNominalSpecialization {
                    ty: raw_index(ty),
                });
            }
            self.nominal_specializations[index] = Some(specialization.id());
            self.nominal_specialization_records.push(specialization);
        }
        self.visiting[index] = false;
        self.identities[index] = Some(identity.clone());
        Ok(identity)
    }

    fn nominal_key(
        &mut self,
        ty: TypeId,
        identity: HirNominalIdentity,
        arguments: &[TypeId],
    ) -> Result<ExactTypeKey, ExactTypeIdentityError> {
        let expected_arguments = identity.source().map_or(0, |source| {
            source
                .declaration()
                .duplicate_signature()
                .type_parameter_count()
        }) as usize;
        if arguments.len() != expected_arguments {
            return Err(ExactTypeIdentityError::NominalArity {
                ty: raw_index(ty),
                expected: expected_arguments,
                actual: arguments.len(),
            });
        }
        match (
            identity.concrete_type_id(),
            identity.generic_type_id(),
            arguments.is_empty(),
        ) {
            (Some(origin), None, true) => Ok(ExactTypeKey::Nominal(origin)),
            (None, Some(origin), false) => Ok(ExactTypeKey::NominalApplication {
                origin,
                arguments: NonEmptyVec::new(self.resolve_children(ty, arguments)?)
                    .expect("generic nominal arguments are non-empty"),
            }),
            _ => Err(ExactTypeIdentityError::NominalIdentityKind { ty: raw_index(ty) }),
        }
    }

    fn function_key(
        &mut self,
        ty: TypeId,
        id: super::FunctionTypeId,
        native: bool,
    ) -> Result<ExactTypeKey, ExactTypeIdentityError> {
        if local_index(id) >= self.inputs.function_types.len() {
            return Err(ExactTypeIdentityError::UnknownReference {
                ty: Some(raw_index(ty)),
                relation: ExactTypeRelation::FunctionType,
                target: raw_index(id),
            });
        }
        let function = self.inputs.function_types[id].clone();
        if local_index(function.canonical_type) >= self.inputs.types.len()
            || !matches!(self.inputs.types[function.canonical_type].kind, TypeKind::Function(actual) if actual == id)
            || (!native && function.canonical_type != ty)
            || (native && function.is_suspend)
        {
            return Err(ExactTypeIdentityError::InvalidFunctionType { ty: raw_index(ty) });
        }
        let parameters = self.resolve_children(ty, &function.parameter_types)?;
        self.require_type(
            Some(ty),
            ExactTypeRelation::FunctionResult,
            function.return_type,
        )?;
        let result = self.resolve(function.return_type)?.id();
        Ok(if native {
            ExactTypeKey::NativeFunctionPointer {
                calling_convention: CallingConvention::C,
                parameters,
                result,
            }
        } else {
            ExactTypeKey::Function {
                effect: if function.is_suspend {
                    Effect::Suspend
                } else {
                    Effect::Ordinary
                },
                parameters,
                result,
            }
        })
    }

    fn resolve_children(
        &mut self,
        parent: TypeId,
        children: &[TypeId],
    ) -> Result<Vec<PersistentExactTypeId>, ExactTypeIdentityError> {
        children
            .iter()
            .map(|child| {
                self.require_type(Some(parent), ExactTypeRelation::ChildType, *child)?;
                self.resolve(*child).map(|identity| identity.id())
            })
            .collect()
    }

    fn require_type(
        &self,
        parent: Option<TypeId>,
        relation: ExactTypeRelation,
        id: TypeId,
    ) -> Result<usize, ExactTypeIdentityError> {
        require_reference(self.inputs.types, parent, relation, id)
    }

    fn require_canonical_nominal(
        &self,
        ty: TypeId,
        canonical: TypeId,
        relation: ExactTypeRelation,
    ) -> Result<(), ExactTypeIdentityError> {
        if canonical != ty {
            return Err(ExactTypeIdentityError::NonCanonicalNominalType {
                ty: raw_index(ty),
                relation,
                canonical: raw_index(canonical),
            });
        }
        Ok(())
    }

    fn require_struct(
        &self,
        parent: Option<TypeId>,
        relation: ExactTypeRelation,
        id: StructId,
    ) -> Result<usize, ExactTypeIdentityError> {
        require_reference(self.inputs.structs, parent, relation, id)
    }

    fn require_enum(&self, parent: TypeId, id: EnumId) -> Result<usize, ExactTypeIdentityError> {
        require_reference(self.inputs.enums, Some(parent), ExactTypeRelation::Enum, id)
    }

    fn require_class(
        &self,
        parent: Option<TypeId>,
        relation: ExactTypeRelation,
        id: ClassId,
    ) -> Result<usize, ExactTypeIdentityError> {
        require_reference(self.inputs.classes, parent, relation, id)
    }

    fn require_interface(
        &self,
        parent: TypeId,
        id: InterfaceId,
    ) -> Result<usize, ExactTypeIdentityError> {
        require_reference(
            self.inputs.interfaces,
            Some(parent),
            ExactTypeRelation::Interface,
            id,
        )
    }
}

fn require_reference<T>(
    arena: &Arena<T>,
    parent: Option<TypeId>,
    relation: ExactTypeRelation,
    id: Idx<T>,
) -> Result<usize, ExactTypeIdentityError> {
    let index = local_index(id);
    if index < arena.len() {
        Ok(index)
    } else {
        Err(ExactTypeIdentityError::UnknownReference {
            ty: parent.map(raw_index),
            relation,
            target: raw_index(id),
        })
    }
}

fn local_index<T>(id: Idx<T>) -> usize {
    raw_index(id) as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
