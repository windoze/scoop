use std::collections::HashSet;
use std::fmt;

use scoop_hir as hir;
use scoop_identity::{
    CallingConvention, CoreBuiltinNominal, Effect, NonEmptyVec, SignatureTypeKey,
};

use crate::{Lowerer, Owner};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SignatureBinder {
    pub(crate) parameter: hir::TypeParamId,
    pub(crate) depth: u32,
    pub(crate) index: u32,
}

pub(crate) struct SignatureTypeMapper<'a> {
    lowerer: &'a Lowerer,
    nominals: &'a hir::HirNominalIdentities,
    intrinsic_core: &'a hir::IntrinsicTypeCore,
}

impl<'a> SignatureTypeMapper<'a> {
    pub(crate) const fn new(
        lowerer: &'a Lowerer,
        nominals: &'a hir::HirNominalIdentities,
        intrinsic_core: &'a hir::IntrinsicTypeCore,
    ) -> Self {
        Self {
            lowerer,
            nominals,
            intrinsic_core,
        }
    }

    pub(crate) fn map(
        &self,
        ty: hir::TypeId,
        binders: &[SignatureBinder],
    ) -> Result<SignatureTypeKey, SignatureTypeMappingError> {
        self.map_inner(ty, binders, &mut HashSet::new())
    }

    fn map_inner(
        &self,
        ty: hir::TypeId,
        binders: &[SignatureBinder],
        visiting: &mut HashSet<hir::TypeId>,
    ) -> Result<SignatureTypeKey, SignatureTypeMappingError> {
        if !visiting.insert(ty) {
            return Err(SignatureTypeMappingError::RecursiveType(raw_index(ty)));
        }
        let key = match &self.lowerer.types[ty] {
            hir::Type::Unit => {
                SignatureTypeKey::Nominal(self.nominals.core_builtin(CoreBuiltinNominal::Unit).id())
            }
            hir::Type::Integer(kind) => self.map_nominal(
                Owner::Struct(self.intrinsic_core.integers.owner(*kind)),
                &[],
                binders,
                visiting,
            )?,
            hir::Type::Boolean => self.map_nominal(
                Owner::Struct(self.intrinsic_core.boolean),
                &[],
                binders,
                visiting,
            )?,
            hir::Type::String => self.map_nominal(
                Owner::Class(self.intrinsic_core.string),
                &[],
                binders,
                visiting,
            )?,
            hir::Type::Struct(application) => {
                let application = &self.lowerer.struct_applications[*application];
                self.map_nominal(
                    Owner::Struct(application.template),
                    &application.arguments,
                    binders,
                    visiting,
                )?
            }
            hir::Type::Class(application) => {
                let application = &self.lowerer.class_applications[*application];
                if let Some(object) = self
                    .lowerer
                    .object_by_backing_class
                    .get(&application.template)
                {
                    self.map_nominal(
                        Owner::Object(*object),
                        &application.arguments,
                        binders,
                        visiting,
                    )?
                } else {
                    self.map_nominal(
                        Owner::Class(application.template),
                        &application.arguments,
                        binders,
                        visiting,
                    )?
                }
            }
            hir::Type::Interface(application) => {
                let application = &self.lowerer.interface_applications[*application];
                self.map_nominal(
                    Owner::Interface(application.template),
                    &application.arguments,
                    binders,
                    visiting,
                )?
            }
            hir::Type::Any => {
                SignatureTypeKey::Nominal(self.nominals.core_builtin(CoreBuiltinNominal::Any).id())
            }
            hir::Type::Tuple(elements) => SignatureTypeKey::Tuple(
                NonEmptyVec::new(self.map_all(elements, binders, visiting)?)
                    .map_err(|_| SignatureTypeMappingError::EmptyTuple)?,
            ),
            hir::Type::Function(function) => {
                let function = &self.lowerer.function_types[*function];
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
            hir::Type::Ptr(pointee) => {
                SignatureTypeKey::RawPointer(Box::new(self.map_inner(*pointee, binders, visiting)?))
            }
            hir::Type::FunPtr(function) => {
                let function = &self.lowerer.function_types[*function];
                SignatureTypeKey::NativeFunctionPointer {
                    calling_convention: CallingConvention::C,
                    parameters: self.map_all(&function.parameter_types, binders, visiting)?,
                    result: Box::new(self.map_inner(function.return_type, binders, visiting)?),
                }
            }
            hir::Type::Enum(application) => {
                let application = &self.lowerer.enum_applications[*application];
                self.map_nominal(
                    Owner::Enum(application.template),
                    &application.arguments,
                    binders,
                    visiting,
                )?
            }
            hir::Type::Param(parameter) => {
                let binder = binders
                    .iter()
                    .find(|binder| binder.parameter == *parameter)
                    .ok_or_else(|| {
                        SignatureTypeMappingError::UnknownBinder(parameter.identity_raw())
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

    fn map_nominal(
        &self,
        owner: Owner,
        arguments: &[hir::TypeId],
        binders: &[SignatureBinder],
        visiting: &mut HashSet<hir::TypeId>,
    ) -> Result<SignatureTypeKey, SignatureTypeMappingError> {
        let identity = match owner {
            Owner::Struct(id) => &self.nominals[id],
            Owner::Enum(id) => &self.nominals[id],
            Owner::Class(id) => &self.nominals[id],
            Owner::Interface(id) => &self.nominals[id],
            Owner::Object(id) => &self.nominals[id],
        };
        let Some(source) = identity.source() else {
            return Err(SignatureTypeMappingError::GeneratedNominal);
        };
        match source {
            hir::HirSourceNominalIdentity::Concrete(record) => {
                if arguments.is_empty() {
                    Ok(SignatureTypeKey::Nominal(record.id()))
                } else {
                    Err(SignatureTypeMappingError::ConcreteNominalArguments)
                }
            }
            hir::HirSourceNominalIdentity::Generic(record) => {
                let arguments = NonEmptyVec::new(self.map_all(arguments, binders, visiting)?)
                    .map_err(|_| SignatureTypeMappingError::MissingGenericArguments)?;
                Ok(SignatureTypeKey::NominalApplication {
                    origin: record.id(),
                    arguments,
                })
            }
        }
    }

    fn map_all(
        &self,
        types: &[hir::TypeId],
        binders: &[SignatureBinder],
        visiting: &mut HashSet<hir::TypeId>,
    ) -> Result<Vec<SignatureTypeKey>, SignatureTypeMappingError> {
        types
            .iter()
            .map(|ty| self.map_inner(*ty, binders, visiting))
            .collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SignatureTypeMappingError {
    RecursiveType(u32),
    UnknownBinder(u32),
    EmptyTuple,
    ConcreteNominalArguments,
    MissingGenericArguments,
    GeneratedNominal,
}

impl fmt::Display for SignatureTypeMappingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecursiveType(ty) => write!(formatter, "type {ty} recursively contains itself"),
            Self::UnknownBinder(parameter) => {
                write!(
                    formatter,
                    "type parameter {parameter} is outside the declaration binder"
                )
            }
            Self::EmptyTuple => formatter.write_str("a signature tuple must be non-empty"),
            Self::ConcreteNominalArguments => {
                formatter.write_str("a concrete nominal signature type has type arguments")
            }
            Self::MissingGenericArguments => {
                formatter.write_str("a generic nominal signature type has no type arguments")
            }
            Self::GeneratedNominal => formatter
                .write_str("a generated nominal cannot replace a source nominal in a signature"),
        }
    }
}

impl std::error::Error for SignatureTypeMappingError {}

fn raw_index<T>(id: la_arena::Idx<T>) -> u32 {
    id.into_raw().into_u32()
}
