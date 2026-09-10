use std::collections::{HashMap, HashSet};
use std::ops::Index;

use la_arena::{Arena, Idx};
use scoop_identity::{
    CallingConvention, CborIdentityRecord, CoreBuiltinNominal, Effect, ExactTypeKey, NonEmptyVec,
    PersistentExactTypeId,
};

use super::{
    ClassDef, ClassId, EnumDef, EnumId, FunctionType, InterfaceDef, InterfaceId, IntrinsicTypeCore,
    ObjectDecl, StructDef, StructId, Type, TypeId, TypeKind,
};
use crate::{
    ClassId as ExportClassId, EnumId as ExportEnumId, HirNominalIdentities, HirNominalIdentity,
    InterfaceId as ExportInterfaceId, ObjectId as ExportObjectId, StructId as ExportStructId,
};

mod error;
pub use error::{ConcreteNominalKind, ExactTypeIdentityError, ExactTypeRelation};

#[cfg(test)]
mod tests;

type ExactTypeRecord = CborIdentityRecord<PersistentExactTypeId, ExactTypeKey>;

#[derive(Clone, Copy)]
pub struct ExactTypeIdentityInputs<'a> {
    pub types: &'a Arena<Type>,
    pub function_types: &'a Arena<FunctionType>,
    pub structs: &'a Arena<StructDef>,
    pub enums: &'a Arena<EnumDef>,
    pub classes: &'a Arena<ClassDef>,
    pub interfaces: &'a Arena<InterfaceDef>,
    pub objects: &'a Arena<ObjectDecl>,
    pub intrinsic_core: &'a IntrinsicTypeCore,
    pub source_nominal_identities: &'a HirNominalIdentities,
}

/// Total persistent exact-type relation for LocalConcrete HIR.
#[derive(Clone, Debug)]
pub struct ExactTypeIdentities {
    identities: Vec<ExactTypeRecord>,
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
    object_by_backing_class: HashMap<ClassId, crate::ObjectId>,
}

impl<'a> ExactTypeIdentityBuilder<'a> {
    fn new(inputs: ExactTypeIdentityInputs<'a>) -> Self {
        Self {
            identities: vec![None; inputs.types.len()],
            visiting: vec![false; inputs.types.len()],
            exact_ids: HashSet::new(),
            object_by_backing_class: HashMap::new(),
            inputs,
        }
    }

    fn build(mut self) -> Result<ExactTypeIdentities, ExactTypeIdentityError> {
        for (object, declaration) in self.inputs.objects.iter() {
            self.require_class(
                None,
                ExactTypeRelation::ObjectBackingClass,
                declaration.backing_class,
            )?;
            if self
                .object_by_backing_class
                .insert(
                    declaration.backing_class,
                    export_object_id(declaration.origin),
                )
                .is_some()
            {
                return Err(ExactTypeIdentityError::DuplicateObjectBackingClass {
                    class: raw_index(declaration.backing_class),
                });
            }
            if raw_index(object) != declaration.origin.into_raw() {
                return Err(ExactTypeIdentityError::InvalidObjectOrigin {
                    object: raw_index(object),
                    origin: declaration.origin.into_raw(),
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
        Ok(ExactTypeIdentities { identities })
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
            TypeKind::Unit => ExactTypeKey::Nominal(
                self.inputs
                    .source_nominal_identities
                    .core_builtin(CoreBuiltinNominal::Unit)
                    .id(),
            ),
            TypeKind::Integer(kind) => {
                let owner = self.inputs.intrinsic_core.integers.owner(kind);
                self.require_struct(Some(ty), ExactTypeRelation::IntrinsicNominalOwner, owner)?;
                let identity = self.source_struct_identity(self.inputs.structs[owner].origin)?;
                self.nominal_key(ty, identity, &[])?
            }
            TypeKind::Boolean => {
                let owner = self.inputs.intrinsic_core.boolean;
                self.require_struct(Some(ty), ExactTypeRelation::IntrinsicNominalOwner, owner)?;
                let identity = self.source_struct_identity(self.inputs.structs[owner].origin)?;
                self.nominal_key(ty, identity, &[])?
            }
            TypeKind::String => {
                let owner = self.inputs.intrinsic_core.string;
                self.require_class(Some(ty), ExactTypeRelation::IntrinsicNominalOwner, owner)?;
                let identity = self.source_class_identity(self.inputs.classes[owner].origin)?;
                self.nominal_key(ty, identity, &[])?
            }
            TypeKind::Any => ExactTypeKey::Nominal(
                self.inputs
                    .source_nominal_identities
                    .core_builtin(CoreBuiltinNominal::Any)
                    .id(),
            ),
            TypeKind::Struct(id) => {
                self.require_struct(Some(ty), ExactTypeRelation::Struct, id)?;
                let declaration = &self.inputs.structs[id];
                let identity = self.source_struct_identity(declaration.origin)?;
                let arguments = declaration.type_arguments.clone();
                self.nominal_key(ty, identity, &arguments)?
            }
            TypeKind::Enum(id) => {
                self.require_enum(ty, id)?;
                let declaration = &self.inputs.enums[id];
                let identity = self.source_enum_identity(declaration.origin)?;
                let arguments = declaration.type_arguments.clone();
                self.nominal_key(ty, identity, &arguments)?
            }
            TypeKind::Class(id) => {
                self.require_class(Some(ty), ExactTypeRelation::Class, id)?;
                let declaration = &self.inputs.classes[id];
                if let Some(object) = self.object_by_backing_class.get(&id).copied() {
                    let identity = self.source_object_identity(object)?;
                    let arguments = declaration.type_arguments.clone();
                    self.nominal_key(ty, identity, &arguments)?
                } else {
                    let identity = self.source_class_identity(declaration.origin)?;
                    let arguments = declaration.type_arguments.clone();
                    self.nominal_key(ty, identity, &arguments)?
                }
            }
            TypeKind::Interface(id) => {
                self.require_interface(ty, id)?;
                let declaration = &self.inputs.interfaces[id];
                let identity = self.source_interface_identity(declaration.origin)?;
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

    fn source_struct_identity(
        &self,
        origin: super::StructOriginId,
    ) -> Result<HirNominalIdentity, ExactTypeIdentityError> {
        self.inputs
            .source_nominal_identities
            .get_struct(export_struct_id(origin))
            .cloned()
            .ok_or(ExactTypeIdentityError::UnknownNominalOrigin {
                kind: ConcreteNominalKind::Struct,
                origin: origin.into_raw(),
            })
    }

    fn source_enum_identity(
        &self,
        origin: super::EnumOriginId,
    ) -> Result<HirNominalIdentity, ExactTypeIdentityError> {
        self.inputs
            .source_nominal_identities
            .get_enum(export_enum_id(origin))
            .cloned()
            .ok_or(ExactTypeIdentityError::UnknownNominalOrigin {
                kind: ConcreteNominalKind::Enum,
                origin: origin.into_raw(),
            })
    }

    fn source_class_identity(
        &self,
        origin: super::ClassOriginId,
    ) -> Result<HirNominalIdentity, ExactTypeIdentityError> {
        self.inputs
            .source_nominal_identities
            .get_class(export_class_id(origin))
            .cloned()
            .ok_or(ExactTypeIdentityError::UnknownNominalOrigin {
                kind: ConcreteNominalKind::Class,
                origin: origin.into_raw(),
            })
    }

    fn source_interface_identity(
        &self,
        origin: super::InterfaceOriginId,
    ) -> Result<HirNominalIdentity, ExactTypeIdentityError> {
        self.inputs
            .source_nominal_identities
            .get_interface(export_interface_id(origin))
            .cloned()
            .ok_or(ExactTypeIdentityError::UnknownNominalOrigin {
                kind: ConcreteNominalKind::Interface,
                origin: origin.into_raw(),
            })
    }

    fn source_object_identity(
        &self,
        origin: ExportObjectId,
    ) -> Result<HirNominalIdentity, ExactTypeIdentityError> {
        self.inputs
            .source_nominal_identities
            .get_object(origin)
            .cloned()
            .ok_or(ExactTypeIdentityError::UnknownNominalOrigin {
                kind: ConcreteNominalKind::Object,
                origin: raw_index(origin),
            })
    }

    fn require_type(
        &self,
        parent: Option<TypeId>,
        relation: ExactTypeRelation,
        id: TypeId,
    ) -> Result<usize, ExactTypeIdentityError> {
        require_reference(self.inputs.types, parent, relation, id)
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

fn export_struct_id(id: super::StructOriginId) -> ExportStructId {
    ExportStructId::from_raw(id.into_raw().into())
}

fn export_enum_id(id: super::EnumOriginId) -> ExportEnumId {
    ExportEnumId::from_raw(id.into_raw().into())
}

fn export_class_id(id: super::ClassOriginId) -> ExportClassId {
    ExportClassId::from_raw(id.into_raw().into())
}

fn export_interface_id(id: super::InterfaceOriginId) -> ExportInterfaceId {
    ExportInterfaceId::from_raw(id.into_raw().into())
}

fn export_object_id(id: super::ObjectOriginId) -> ExportObjectId {
    ExportObjectId::from_raw(id.into_raw().into())
}

fn local_index<T>(id: Idx<T>) -> usize {
    raw_index(id) as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
