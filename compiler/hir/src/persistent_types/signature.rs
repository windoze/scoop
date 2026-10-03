use std::collections::HashSet;

use scoop_identity::{
    CallingConvention, CoreBuiltinNominal, Effect, NonEmptyVec, SignatureTypeKey,
};

use super::HirTypeIdentityInputs;
use crate::{HirNominalIdentity, HirSourceNominalIdentity, ObjectId, Type, TypeId, TypeParamId};

mod error;
pub use error::HirSignatureTypeMappingError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirSignatureBinder {
    pub parameter: TypeParamId,
    pub depth: u32,
    pub index: u32,
}

/// Maps a complete HIR type tree to the source duplicate-signature schema.
pub struct HirSignatureTypeMapper<'a> {
    inputs: HirTypeIdentityInputs<'a>,
}

impl<'a> HirSignatureTypeMapper<'a> {
    pub const fn new(inputs: HirTypeIdentityInputs<'a>) -> Self {
        Self { inputs }
    }

    pub fn map(
        &self,
        ty: TypeId,
        binders: &[HirSignatureBinder],
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        self.map_inner(ty, binders, &mut HashSet::new())
    }

    fn map_inner(
        &self,
        ty: TypeId,
        binders: &[HirSignatureBinder],
        visiting: &mut HashSet<TypeId>,
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        if local_index(ty) >= self.inputs.types.len() {
            return Err(HirSignatureTypeMappingError::UnknownType(raw_index(ty)));
        }
        if !visiting.insert(ty) {
            return Err(HirSignatureTypeMappingError::RecursiveType(raw_index(ty)));
        }
        let key = match &self.inputs.types[ty] {
            Type::Unit => SignatureTypeKey::Nominal(
                self.inputs
                    .nominal_identities
                    .core_builtin(CoreBuiltinNominal::Unit)
                    .id(),
            ),
            Type::Integer(kind) => match self.inputs.core_types {
                super::HirCoreTypeIdentityAuthority::Defined(core) => {
                    self.map_struct(core.integers.owner(*kind), &[], binders, visiting)?
                }
                super::HirCoreTypeIdentityAuthority::Imported(core) => {
                    SignatureTypeKey::Nominal(core.integer(*kind).persistent())
                }
            },
            Type::Boolean => match self.inputs.core_types {
                super::HirCoreTypeIdentityAuthority::Defined(core) => {
                    self.map_struct(core.boolean, &[], binders, visiting)?
                }
                super::HirCoreTypeIdentityAuthority::Imported(core) => {
                    SignatureTypeKey::Nominal(core.boolean().persistent())
                }
            },
            Type::String => match self.inputs.core_types {
                super::HirCoreTypeIdentityAuthority::Defined(core) => {
                    self.map_class(core.string, &[], binders, visiting)?
                }
                super::HirCoreTypeIdentityAuthority::Imported(core) => {
                    SignatureTypeKey::Nominal(core.string().persistent())
                }
            },
            Type::Struct(application) => {
                if local_index(*application) >= self.inputs.struct_applications.len() {
                    return Err(HirSignatureTypeMappingError::UnknownApplication(raw_index(
                        *application,
                    )));
                }
                let application = &self.inputs.struct_applications[*application];
                let (identity, parameter_count) = self
                    .inputs
                    .struct_declaration(application.template)
                    .ok_or_else(|| {
                        HirSignatureTypeMappingError::InvalidApplication(raw_index(ty))
                    })?;
                if application.canonical_type != ty {
                    return Err(HirSignatureTypeMappingError::InvalidApplication(raw_index(
                        ty,
                    )));
                }
                self.map_nominal(
                    &identity,
                    parameter_count,
                    &application.arguments,
                    binders,
                    visiting,
                )?
            }
            Type::Class(application) => {
                if local_index(*application) >= self.inputs.class_applications.len() {
                    return Err(HirSignatureTypeMappingError::UnknownApplication(raw_index(
                        *application,
                    )));
                }
                let application = &self.inputs.class_applications[*application];

                if application.canonical_type != ty {
                    return Err(HirSignatureTypeMappingError::InvalidApplication(raw_index(
                        ty,
                    )));
                }
                if let Some(template) = self
                    .inputs
                    .nominal_identities
                    .class_id(application.template)
                {
                    self.map_class(template, &application.arguments, binders, visiting)?
                } else {
                    let (identity, arity) = self
                        .inputs
                        .class_declaration(application.template)
                        .ok_or_else(|| {
                            HirSignatureTypeMappingError::InvalidApplication(raw_index(ty))
                        })?;
                    self.map_nominal(&identity, arity, &application.arguments, binders, visiting)?
                }
            }
            Type::Interface(application) => {
                if local_index(*application) >= self.inputs.interface_applications.len() {
                    return Err(HirSignatureTypeMappingError::UnknownApplication(raw_index(
                        *application,
                    )));
                }
                let application = &self.inputs.interface_applications[*application];
                let (identity, parameter_count) = self
                    .inputs
                    .interface_declaration(application.template)
                    .ok_or_else(|| {
                        HirSignatureTypeMappingError::InvalidApplication(raw_index(ty))
                    })?;
                if application.canonical_type != ty {
                    return Err(HirSignatureTypeMappingError::InvalidApplication(raw_index(
                        ty,
                    )));
                }
                self.map_nominal(
                    &identity,
                    parameter_count,
                    &application.arguments,
                    binders,
                    visiting,
                )?
            }
            Type::Any => SignatureTypeKey::Nominal(
                self.inputs
                    .nominal_identities
                    .core_builtin(CoreBuiltinNominal::Any)
                    .id(),
            ),
            Type::Tuple(elements) => SignatureTypeKey::Tuple(
                NonEmptyVec::new(self.map_all(elements, binders, visiting)?)
                    .map_err(|_| HirSignatureTypeMappingError::EmptyTuple)?,
            ),
            Type::Function(function) => {
                let function = self.function(ty, *function, false)?;
                SignatureTypeKey::Function {
                    effect: if function.is_suspend {
                        Effect::Suspend
                    } else {
                        Effect::Ordinary
                    },
                    parameters: self.map_all(&function.parameter_types, binders, visiting)?,
                    result: Box::new(self.map_inner(function.return_type, binders, visiting)?),
                }
            }
            Type::Ptr(pointee) => {
                SignatureTypeKey::RawPointer(Box::new(self.map_inner(*pointee, binders, visiting)?))
            }
            Type::FunPtr(function) => {
                let function = self.function(ty, *function, true)?;
                if function.is_suspend {
                    return Err(HirSignatureTypeMappingError::SuspendNativeFunctionPointer);
                }
                SignatureTypeKey::NativeFunctionPointer {
                    calling_convention: CallingConvention::C,
                    parameters: self.map_all(&function.parameter_types, binders, visiting)?,
                    result: Box::new(self.map_inner(function.return_type, binders, visiting)?),
                }
            }
            Type::Enum(application) => {
                if local_index(*application) >= self.inputs.enum_applications.len() {
                    return Err(HirSignatureTypeMappingError::UnknownApplication(raw_index(
                        *application,
                    )));
                }
                let application = &self.inputs.enum_applications[*application];
                let (identity, parameter_count) = self
                    .inputs
                    .enum_declaration(application.template)
                    .ok_or_else(|| {
                        HirSignatureTypeMappingError::InvalidApplication(raw_index(ty))
                    })?;
                if application.canonical_type != ty {
                    return Err(HirSignatureTypeMappingError::InvalidApplication(raw_index(
                        ty,
                    )));
                }
                self.map_nominal(
                    &identity,
                    parameter_count,
                    &application.arguments,
                    binders,
                    visiting,
                )?
            }
            Type::Param(parameter) => {
                let binder = binders
                    .iter()
                    .find(|binder| binder.parameter == *parameter)
                    .ok_or_else(|| {
                        HirSignatureTypeMappingError::UnknownBinder(parameter.identity_raw())
                    })?;
                SignatureTypeKey::Binder {
                    depth: binder.depth,
                    index: binder.index,
                }
            }
        };
        visiting.remove(&ty);
        Ok(key)
    }

    fn map_struct(
        &self,
        owner: crate::StructId,
        arguments: &[TypeId],
        binders: &[HirSignatureBinder],
        visiting: &mut HashSet<TypeId>,
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        if local_index(owner) >= self.inputs.structs.len() {
            return Err(HirSignatureTypeMappingError::UnknownNominal(raw_index(
                owner,
            )));
        }
        self.map_nominal(
            &self.inputs.nominal_identities[owner],
            self.inputs.structs[owner].type_params.len(),
            arguments,
            binders,
            visiting,
        )
    }

    fn map_class(
        &self,
        owner: crate::ClassId,
        arguments: &[TypeId],
        binders: &[HirSignatureBinder],
        visiting: &mut HashSet<TypeId>,
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        if local_index(owner) >= self.inputs.classes.len() {
            return Err(HirSignatureTypeMappingError::UnknownNominal(raw_index(
                owner,
            )));
        }
        let mut objects = self
            .inputs
            .objects
            .iter()
            .filter_map(|(object, declaration)| {
                (declaration.backing_class == owner).then_some(object)
            });
        let object = objects.next();
        if objects.next().is_some() {
            return Err(HirSignatureTypeMappingError::DuplicateObjectBackingClass(
                raw_index(owner),
            ));
        }
        let (identity, arity) = match object {
            Some(object) => (self.object_identity(object)?, 0),
            None => (
                &self.inputs.nominal_identities[owner],
                self.inputs.classes[owner].type_params.len(),
            ),
        };
        self.map_nominal(identity, arity, arguments, binders, visiting)
    }

    fn object_identity(
        &self,
        object: ObjectId,
    ) -> Result<&HirNominalIdentity, HirSignatureTypeMappingError> {
        if local_index(object) >= self.inputs.objects.len() {
            return Err(HirSignatureTypeMappingError::UnknownNominal(raw_index(
                object,
            )));
        }
        Ok(&self.inputs.nominal_identities[object])
    }

    fn map_nominal(
        &self,
        identity: &HirNominalIdentity,
        arity: usize,
        arguments: &[TypeId],
        binders: &[HirSignatureBinder],
        visiting: &mut HashSet<TypeId>,
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        if arguments.len() != arity {
            return Err(HirSignatureTypeMappingError::NominalArity {
                expected: arity,
                actual: arguments.len(),
            });
        }
        let Some(source) = identity.source() else {
            return Err(HirSignatureTypeMappingError::GeneratedNominal);
        };
        match source {
            HirSourceNominalIdentity::Concrete(record) => {
                if arity == 0 {
                    Ok(SignatureTypeKey::Nominal(record.id()))
                } else {
                    Err(HirSignatureTypeMappingError::NominalIdentityKind)
                }
            }
            HirSourceNominalIdentity::Generic(record) => {
                if arity == 0 {
                    return Err(HirSignatureTypeMappingError::NominalIdentityKind);
                }
                let arguments = NonEmptyVec::new(self.map_all(arguments, binders, visiting)?)
                    .expect("a generic nominal with positive arity has arguments");
                Ok(SignatureTypeKey::NominalApplication {
                    origin: record.id(),
                    arguments,
                })
            }
        }
    }

    fn map_all(
        &self,
        types: &[TypeId],
        binders: &[HirSignatureBinder],
        visiting: &mut HashSet<TypeId>,
    ) -> Result<Vec<SignatureTypeKey>, HirSignatureTypeMappingError> {
        types
            .iter()
            .map(|ty| self.map_inner(*ty, binders, visiting))
            .collect()
    }

    fn function(
        &self,
        ty: TypeId,
        function: crate::FunctionTypeId,
        native: bool,
    ) -> Result<&crate::FunctionType, HirSignatureTypeMappingError> {
        if local_index(function) >= self.inputs.function_types.len() {
            return Err(HirSignatureTypeMappingError::UnknownFunctionType(
                raw_index(function),
            ));
        }
        let function_value = &self.inputs.function_types[function];
        if local_index(function_value.canonical_type) >= self.inputs.types.len()
            || !matches!(
                self.inputs.types[function_value.canonical_type],
                Type::Function(found) if found == function
            )
            || (!native && function_value.canonical_type != ty)
        {
            return Err(HirSignatureTypeMappingError::InvalidFunctionType(
                raw_index(ty),
            ));
        }
        Ok(function_value)
    }
}

fn local_index<T>(id: la_arena::Idx<T>) -> usize {
    id.into_raw().into_u32() as usize
}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
