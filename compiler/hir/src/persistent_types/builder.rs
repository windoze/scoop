use std::collections::{HashMap, HashSet};

use la_arena::Idx;
use scoop_identity::{
    CallingConvention, CborIdentityRecord, CoreBuiltinNominal, Effect, ExactTypeKey, NonEmptyVec,
    PersistentExactTypeId,
};

use crate::{
    ClassApplicationId, ClassId, HirNominalIdentity, HirSourceNominalIdentity, Type, TypeId,
    TypeParamId,
};

use super::{
    HirOpenTypeIdentity, HirTypeIdentities, HirTypeIdentity, HirTypeIdentityError,
    HirTypeIdentityInputs, HirTypeRelation,
};

pub(super) fn build(
    inputs: HirTypeIdentityInputs<'_>,
) -> Result<HirTypeIdentities, HirTypeIdentityError> {
    TypeIdentityBuilder::new(inputs).build()
}

struct TypeIdentityBuilder<'a> {
    inputs: HirTypeIdentityInputs<'a>,
    identities: Vec<Option<HirTypeIdentity>>,
    visiting: Vec<bool>,
    exact_ids: HashSet<PersistentExactTypeId>,
    object_by_backing_class: HashMap<ClassId, crate::ObjectId>,
}

impl<'a> TypeIdentityBuilder<'a> {
    fn new(inputs: HirTypeIdentityInputs<'a>) -> Self {
        Self {
            identities: vec![None; inputs.types.len()],
            visiting: vec![false; inputs.types.len()],
            exact_ids: HashSet::new(),
            object_by_backing_class: HashMap::new(),
            inputs,
        }
    }

    fn build(mut self) -> Result<HirTypeIdentities, HirTypeIdentityError> {
        for (object, declaration) in self.inputs.objects.iter() {
            if local_index(declaration.backing_class) >= self.inputs.classes.len() {
                return Err(HirTypeIdentityError::UnknownReference {
                    ty: None,
                    relation: HirTypeRelation::ObjectBackingClass,
                    target: raw_index(declaration.backing_class),
                });
            }
            if self
                .object_by_backing_class
                .insert(declaration.backing_class, object)
                .is_some()
            {
                return Err(HirTypeIdentityError::DuplicateObjectBackingClass {
                    class: raw_index(declaration.backing_class),
                });
            }
        }

        let type_ids = self
            .inputs
            .types
            .iter()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        for id in type_ids {
            self.resolve(id)?;
        }
        let identities = self
            .identities
            .into_iter()
            .enumerate()
            .map(|(index, identity)| {
                identity.ok_or(HirTypeIdentityError::MissingIdentity { ty: index as u32 })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(HirTypeIdentities { identities })
    }

    fn resolve(&mut self, ty: TypeId) -> Result<HirTypeIdentity, HirTypeIdentityError> {
        let index = self.require_type(None, HirTypeRelation::ChildType, ty)?;
        if let Some(identity) = &self.identities[index] {
            return Ok(identity.clone());
        }
        if std::mem::replace(&mut self.visiting[index], true) {
            return Err(HirTypeIdentityError::Cycle { ty: raw_index(ty) });
        }

        let value = self.inputs.types[ty].clone();
        let identity = match value {
            Type::Unit => self.nominal_exact(
                ty,
                self.inputs
                    .nominal_identities
                    .core_builtin(CoreBuiltinNominal::Unit)
                    .id(),
            )?,
            Type::Integer(kind) => match self.inputs.core_types {
                super::HirCoreTypeIdentityAuthority::Defined(core) => {
                    let owner = core.integers.owner(kind);
                    self.require_struct(ty, owner)?;
                    self.source_nominal_exact(ty, self.inputs.nominal_identities[owner].clone())?
                }
                super::HirCoreTypeIdentityAuthority::Imported(core) => {
                    self.nominal_exact(ty, core.integer(kind).persistent())?
                }
            },
            Type::Boolean => match self.inputs.core_types {
                super::HirCoreTypeIdentityAuthority::Defined(core) => {
                    let owner = core.boolean;
                    self.require_struct(ty, owner)?;
                    self.source_nominal_exact(ty, self.inputs.nominal_identities[owner].clone())?
                }
                super::HirCoreTypeIdentityAuthority::Imported(core) => {
                    self.nominal_exact(ty, core.boolean().persistent())?
                }
            },
            Type::String => match self.inputs.core_types {
                super::HirCoreTypeIdentityAuthority::Defined(core) => {
                    let owner = core.string;
                    self.require_class(ty, owner)?;
                    self.source_nominal_exact(ty, self.inputs.nominal_identities[owner].clone())?
                }
                super::HirCoreTypeIdentityAuthority::Imported(core) => {
                    self.nominal_exact(ty, core.string().persistent())?
                }
            },
            Type::Any => self.nominal_exact(
                ty,
                self.inputs
                    .nominal_identities
                    .core_builtin(CoreBuiltinNominal::Any)
                    .id(),
            )?,
            Type::Struct(application) => self.struct_application(ty, application)?,
            Type::ImportedStruct(structure) => {
                self.nominal_exact(ty, structure.declaration.identity.id())?
            }
            Type::ImportedEnum(structure) => {
                self.nominal_exact(ty, structure.declaration.identity.id())?
            }
            Type::ImportedClass(structure) => {
                self.nominal_exact(ty, structure.declaration.identity.id())?
            }
            Type::Enum(application) => self.enum_application(ty, application)?,
            Type::Class(application) => self.class_application(ty, application)?,
            Type::Interface(application) => self.interface_application(ty, application)?,
            Type::Tuple(elements) => {
                if elements.is_empty() {
                    return Err(HirTypeIdentityError::EmptyTuple { ty: raw_index(ty) });
                }
                match self.resolve_children(ty, &elements)? {
                    ResolvedChildren::Exact(elements) => self.exact_record(
                        ty,
                        ExactTypeKey::Tuple(
                            NonEmptyVec::new(elements)
                                .expect("the HIR tuple was checked as non-empty"),
                        ),
                    )?,
                    ResolvedChildren::Open(parameters) => open_identity(parameters),
                }
            }
            Type::Function(function) => self.function_type(ty, function, false)?,
            Type::Ptr(pointee) => {
                self.require_type(Some(ty), HirTypeRelation::ChildType, pointee)?;
                match self.resolve(pointee)? {
                    HirTypeIdentity::Exact(record) => {
                        self.exact_record(ty, ExactTypeKey::RawPointer(record.id()))?
                    }
                    HirTypeIdentity::Open(open) => HirTypeIdentity::Open(open),
                }
            }
            Type::FunPtr(function) => self.function_type(ty, function, true)?,
            Type::Param(parameter) => open_identity(vec![parameter]),
        };
        self.visiting[index] = false;
        self.identities[index] = Some(identity.clone());
        Ok(identity)
    }

    fn source_nominal_exact(
        &mut self,
        ty: TypeId,
        identity: HirNominalIdentity,
    ) -> Result<HirTypeIdentity, HirTypeIdentityError> {
        match identity {
            HirNominalIdentity::Source(HirSourceNominalIdentity::Concrete(record)) => {
                self.nominal_exact(ty, record.id())
            }
            HirNominalIdentity::Source(HirSourceNominalIdentity::Generic(_))
            | HirNominalIdentity::Generated(_) => {
                Err(HirTypeIdentityError::InvalidIntrinsicNominal { ty: raw_index(ty) })
            }
        }
    }

    fn nominal_exact(
        &mut self,
        ty: TypeId,
        nominal: scoop_identity::PersistentTypeId,
    ) -> Result<HirTypeIdentity, HirTypeIdentityError> {
        self.exact_record(ty, ExactTypeKey::Nominal(nominal))
    }

    fn struct_application(
        &mut self,
        ty: TypeId,
        id: crate::StructApplicationId,
    ) -> Result<HirTypeIdentity, HirTypeIdentityError> {
        if local_index(id) >= self.inputs.struct_applications.len() {
            return self.unknown(ty, HirTypeRelation::StructApplication, id);
        }
        let application = self.inputs.struct_applications[id].clone();
        if application.canonical_type != ty
            || local_index(application.template) >= self.inputs.structs.len()
        {
            return Err(HirTypeIdentityError::InvalidApplication { ty: raw_index(ty) });
        }
        self.nominal_application(
            ty,
            self.inputs.nominal_identities[application.template].clone(),
            self.inputs.structs[application.template].type_params.len(),
            &application.arguments,
        )
    }

    fn enum_application(
        &mut self,
        ty: TypeId,
        id: crate::EnumApplicationId,
    ) -> Result<HirTypeIdentity, HirTypeIdentityError> {
        if local_index(id) >= self.inputs.enum_applications.len() {
            return self.unknown(ty, HirTypeRelation::EnumApplication, id);
        }
        let application = self.inputs.enum_applications[id].clone();
        if application.canonical_type != ty
            || local_index(application.template) >= self.inputs.enums.len()
        {
            return Err(HirTypeIdentityError::InvalidApplication { ty: raw_index(ty) });
        }
        self.nominal_application(
            ty,
            self.inputs.nominal_identities[application.template].clone(),
            self.inputs.enums[application.template].type_params.len(),
            &application.arguments,
        )
    }

    fn class_application(
        &mut self,
        ty: TypeId,
        id: ClassApplicationId,
    ) -> Result<HirTypeIdentity, HirTypeIdentityError> {
        if local_index(id) >= self.inputs.class_applications.len() {
            return self.unknown(ty, HirTypeRelation::ClassApplication, id);
        }
        let application = self.inputs.class_applications[id].clone();
        if application.canonical_type != ty
            || local_index(application.template) >= self.inputs.classes.len()
        {
            return Err(HirTypeIdentityError::InvalidApplication { ty: raw_index(ty) });
        }
        if let Some(&object) = self.object_by_backing_class.get(&application.template) {
            return self.nominal_application(
                ty,
                self.inputs.nominal_identities[object].clone(),
                0,
                &application.arguments,
            );
        }
        self.nominal_application(
            ty,
            self.inputs.nominal_identities[application.template].clone(),
            self.inputs.classes[application.template].type_params.len(),
            &application.arguments,
        )
    }

    fn interface_application(
        &mut self,
        ty: TypeId,
        id: crate::InterfaceApplicationId,
    ) -> Result<HirTypeIdentity, HirTypeIdentityError> {
        if local_index(id) >= self.inputs.interface_applications.len() {
            return self.unknown(ty, HirTypeRelation::InterfaceApplication, id);
        }
        let application = self.inputs.interface_applications[id].clone();
        if application.canonical_type != ty
            || local_index(application.template) >= self.inputs.interfaces.len()
        {
            return Err(HirTypeIdentityError::InvalidApplication { ty: raw_index(ty) });
        }
        self.nominal_application(
            ty,
            self.inputs.nominal_identities[application.template].clone(),
            self.inputs.interfaces[application.template]
                .type_params
                .len(),
            &application.arguments,
        )
    }

    fn nominal_application(
        &mut self,
        ty: TypeId,
        identity: HirNominalIdentity,
        arity: usize,
        arguments: &[TypeId],
    ) -> Result<HirTypeIdentity, HirTypeIdentityError> {
        if arguments.len() != arity {
            return Err(HirTypeIdentityError::NominalArity {
                ty: raw_index(ty),
                expected: arity,
                actual: arguments.len(),
            });
        }
        match identity {
            HirNominalIdentity::Source(HirSourceNominalIdentity::Concrete(record)) => {
                if arity != 0 {
                    return Err(HirTypeIdentityError::NominalIdentityKind { ty: raw_index(ty) });
                }
                self.nominal_exact(ty, record.id())
            }
            HirNominalIdentity::Generated(record) => {
                if arity != 0 {
                    return Err(HirTypeIdentityError::NominalIdentityKind { ty: raw_index(ty) });
                }
                self.nominal_exact(ty, record.id())
            }
            HirNominalIdentity::Source(HirSourceNominalIdentity::Generic(record)) => {
                if arity == 0 {
                    return Err(HirTypeIdentityError::NominalIdentityKind { ty: raw_index(ty) });
                }
                match self.resolve_children(ty, arguments)? {
                    ResolvedChildren::Exact(arguments) => self.exact_record(
                        ty,
                        ExactTypeKey::NominalApplication {
                            origin: record.id(),
                            arguments: NonEmptyVec::new(arguments)
                                .expect("generic nominal arguments are non-empty"),
                        },
                    ),
                    ResolvedChildren::Open(parameters) => Ok(open_identity(parameters)),
                }
            }
        }
    }

    fn function_type(
        &mut self,
        ty: TypeId,
        id: crate::FunctionTypeId,
        native: bool,
    ) -> Result<HirTypeIdentity, HirTypeIdentityError> {
        if local_index(id) >= self.inputs.function_types.len() {
            return self.unknown(ty, HirTypeRelation::FunctionType, id);
        }
        let function = self.inputs.function_types[id].clone();
        if local_index(function.canonical_type) >= self.inputs.types.len()
            || !matches!(self.inputs.types[function.canonical_type], Type::Function(actual) if actual == id)
            || (!native && function.canonical_type != ty)
            || (native && function.is_suspend)
        {
            return Err(HirTypeIdentityError::InvalidFunctionType { ty: raw_index(ty) });
        }
        let mut children = function.parameter_types.clone();
        children.push(function.return_type);
        match self.resolve_children(ty, &children)? {
            ResolvedChildren::Exact(mut identities) => {
                let result = identities
                    .pop()
                    .expect("a function signature always includes its result");
                let key = if native {
                    ExactTypeKey::NativeFunctionPointer {
                        calling_convention: CallingConvention::C,
                        parameters: identities,
                        result,
                    }
                } else {
                    ExactTypeKey::Function {
                        effect: if function.is_suspend {
                            Effect::Suspend
                        } else {
                            Effect::Ordinary
                        },
                        parameters: identities,
                        result,
                    }
                };
                self.exact_record(ty, key)
            }
            ResolvedChildren::Open(parameters) => Ok(open_identity(parameters)),
        }
    }

    fn resolve_children(
        &mut self,
        parent: TypeId,
        children: &[TypeId],
    ) -> Result<ResolvedChildren, HirTypeIdentityError> {
        let mut exact = Vec::with_capacity(children.len());
        let mut open = Vec::new();
        for child in children {
            self.require_type(Some(parent), HirTypeRelation::ChildType, *child)?;
            match self.resolve(*child)? {
                HirTypeIdentity::Exact(record) => exact.push(record.id()),
                HirTypeIdentity::Open(identity) => open.extend(identity.parameters),
            }
        }
        if open.is_empty() {
            Ok(ResolvedChildren::Exact(exact))
        } else {
            Ok(ResolvedChildren::Open(canonical_parameters(parent, open)?))
        }
    }

    fn exact_record(
        &mut self,
        ty: TypeId,
        key: ExactTypeKey,
    ) -> Result<HirTypeIdentity, HirTypeIdentityError> {
        let record = CborIdentityRecord::from_key(key).map_err(|error| {
            HirTypeIdentityError::InvalidIdentity {
                ty: raw_index(ty),
                error,
            }
        })?;
        if !self.exact_ids.insert(record.id()) {
            return Err(HirTypeIdentityError::DuplicateExactIdentity { ty: raw_index(ty) });
        }
        Ok(HirTypeIdentity::Exact(record))
    }

    fn require_type(
        &self,
        parent: Option<TypeId>,
        relation: HirTypeRelation,
        ty: TypeId,
    ) -> Result<usize, HirTypeIdentityError> {
        let index = local_index(ty);
        if index < self.inputs.types.len() {
            Ok(index)
        } else {
            Err(HirTypeIdentityError::UnknownReference {
                ty: parent.map(raw_index),
                relation,
                target: raw_index(ty),
            })
        }
    }

    fn require_struct(
        &self,
        ty: TypeId,
        owner: crate::StructId,
    ) -> Result<(), HirTypeIdentityError> {
        if local_index(owner) < self.inputs.structs.len() {
            Ok(())
        } else {
            Err(HirTypeIdentityError::UnknownReference {
                ty: Some(raw_index(ty)),
                relation: HirTypeRelation::IntrinsicNominalOwner,
                target: raw_index(owner),
            })
        }
    }

    fn require_class(&self, ty: TypeId, owner: ClassId) -> Result<(), HirTypeIdentityError> {
        if local_index(owner) < self.inputs.classes.len() {
            Ok(())
        } else {
            Err(HirTypeIdentityError::UnknownReference {
                ty: Some(raw_index(ty)),
                relation: HirTypeRelation::IntrinsicNominalOwner,
                target: raw_index(owner),
            })
        }
    }

    fn unknown<T>(
        &self,
        ty: TypeId,
        relation: HirTypeRelation,
        id: Idx<T>,
    ) -> Result<HirTypeIdentity, HirTypeIdentityError> {
        Err(HirTypeIdentityError::UnknownReference {
            ty: Some(raw_index(ty)),
            relation,
            target: raw_index(id),
        })
    }
}

enum ResolvedChildren {
    Exact(Vec<PersistentExactTypeId>),
    Open(Vec<TypeParamId>),
}

fn open_identity(parameters: Vec<TypeParamId>) -> HirTypeIdentity {
    assert!(
        !parameters.is_empty(),
        "an open type has at least one binder"
    );
    HirTypeIdentity::Open(HirOpenTypeIdentity { parameters })
}

fn canonical_parameters(
    ty: TypeId,
    mut parameters: Vec<TypeParamId>,
) -> Result<Vec<TypeParamId>, HirTypeIdentityError> {
    parameters.sort_by_key(|parameter| (parameter.identity_raw(), parameter.into_raw()));
    for pair in parameters.windows(2) {
        if pair[0].identity_raw() == pair[1].identity_raw() && pair[0] != pair[1] {
            return Err(HirTypeIdentityError::ConflictingBinderSlot {
                ty: raw_index(ty),
                identity: pair[0].identity_raw(),
            });
        }
    }
    parameters.dedup();
    Ok(parameters)
}

fn local_index<T>(id: Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
