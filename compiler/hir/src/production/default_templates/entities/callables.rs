//! Callable applications, bounded dispatch, and imported-core targets.

use scoop_identity::OptionalSignatureType;

use super::{DefaultEntityProjector, arena_get, unknown};
use crate::{
    Callable, DefaultBinderRefV1, DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1,
    DefaultCallableDeclarationV1, DefaultCallableRefV1, GenericMethodOwner, HirSignatureBinder,
    ImportedCoreCallableUseId, ImportedCorePreludeTarget, ImportedDependencyCallableUseId,
    MethodOwnerApplication, TypeId,
};

impl DefaultEntityProjector<'_, '_> {
    pub(in crate::production::default_templates) fn callable(
        &self,
        callable: Callable,
        binders: &[HirSignatureBinder],
    ) -> Result<DefaultCallableRefV1, super::super::DefaultEntityProjectionError> {
        let (function, owner, arguments) = match callable {
            Callable::Function(function) => (function, None, Vec::new()),
            Callable::Generic(application_id) => {
                let application = arena_get(&self.export.instantiations, application_id).ok_or(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "generic function application",
                        index: super::super::raw_index(application_id),
                    },
                )?;
                let generic = arena_get(&self.export.generic_functions, application.generic)
                    .ok_or(super::super::DefaultEntityProjectionError::Unknown {
                        kind: "generic function",
                        index: super::super::raw_index(application.generic),
                    })?;
                (generic.function, None, application.type_args.clone())
            }
            Callable::Method(application_id) => {
                let application = arena_get(&self.export.method_applications, application_id)
                    .ok_or(super::super::DefaultEntityProjectionError::Unknown {
                        kind: "method application",
                        index: super::super::raw_index(application_id),
                    })?;
                (
                    application.function,
                    Some(self.method_owner_type(application.owner)?),
                    Vec::new(),
                )
            }
            Callable::GenericMethod(application_id) => {
                let application =
                    arena_get(&self.export.generic_method_applications, application_id).ok_or(
                        super::super::DefaultEntityProjectionError::Unknown {
                            kind: "generic method application",
                            index: super::super::raw_index(application_id),
                        },
                    )?;
                let method = arena_get(&self.export.generic_methods, application.method).ok_or(
                    super::super::DefaultEntityProjectionError::Unknown {
                        kind: "generic method",
                        index: super::super::raw_index(application.method),
                    },
                )?;
                (
                    method.function,
                    Some(self.generic_method_owner_type(application.owner)?),
                    application.method_arguments.to_vec(),
                )
            }
        };
        let owner = owner.map(|ty| self.type_key(ty, binders)).transpose()?;
        let arguments = arguments
            .into_iter()
            .map(|ty| self.type_key(ty, binders))
            .collect::<Result<Vec<_>, _>>()?;
        DefaultCallableRefV1::try_new(
            self.callable_declaration(function)?,
            OptionalSignatureType::from_option(owner),
            arguments,
        )
        .map_err(super::super::DefaultEntityProjectionError::Callable)
    }

    pub(in crate::production::default_templates) fn imported_callable(
        &self,
        id: ImportedCoreCallableUseId,
    ) -> Result<DefaultCallableRefV1, super::super::DefaultEntityProjectionError> {
        let index = super::super::raw_index(id);
        let use_ = arena_get(&self.export.imported_core_callables, id).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "imported-core callable use",
                index,
            },
        )?;
        let selected = self
            .imported_core
            .and_then(|set| set.resolve_callable(use_.reference()))
            .ok_or(super::super::DefaultEntityProjectionError::ImportedCoreUnavailable(index))?;
        let ImportedCorePreludeTarget::Callable(target) = selected.target() else {
            return Err(super::super::DefaultEntityProjectionError::ImportedCoreKind(index));
        };
        let declaration = match target.definition() {
            crate::CoreCallableDefinitionV1::Function(id) => {
                DefaultCallableDeclarationV1::Function(id)
            }
            crate::CoreCallableDefinitionV1::GenericFunction(id) => {
                DefaultCallableDeclarationV1::GenericFunction(id)
            }
        };
        DefaultCallableRefV1::try_new(declaration, OptionalSignatureType::Absent, Vec::new())
            .map_err(super::super::DefaultEntityProjectionError::Callable)
    }

    pub(in crate::production::default_templates) fn imported_dependency_callable(
        &self,
        id: ImportedDependencyCallableUseId,
    ) -> Result<DefaultCallableRefV1, super::super::DefaultEntityProjectionError> {
        let index = super::super::raw_index(id);
        let use_ = arena_get(&self.export.imported_dependency_callables, id).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "imported dependency callable use",
                index,
            },
        )?;
        let selected = self
            .imported_dependencies
            .and_then(|set| set.resolve_callable(use_.reference()))
            .ok_or(
                super::super::DefaultEntityProjectionError::ImportedDependencyUnavailable(index),
            )?;
        let declaration = match selected.capability().declaration() {
            scoop_identity::DependencyCallableDeclarationId::Function(id) => {
                DefaultCallableDeclarationV1::Function(id)
            }
            scoop_identity::DependencyCallableDeclarationId::PropertyAccessor(id) => {
                DefaultCallableDeclarationV1::PropertyAccessor(id)
            }
        };
        DefaultCallableRefV1::try_new(declaration, OptionalSignatureType::Absent, Vec::new())
            .map_err(super::super::DefaultEntityProjectionError::Callable)
    }

    pub(in crate::production::default_templates) fn bound_callable(
        &self,
        id: crate::BoundCallableRefId,
        binders: &[HirSignatureBinder],
    ) -> Result<DefaultBoundCallableRefV1, super::super::DefaultEntityProjectionError> {
        let bound = arena_get(&self.export.bound_callable_refs, id).ok_or(
            super::super::DefaultEntityProjectionError::Unknown {
                kind: "bound callable",
                index: super::super::raw_index(id),
            },
        )?;
        let receiver = binders
            .iter()
            .find(|binder| binder.parameter == bound.receiver_parameter)
            .ok_or(
                super::super::DefaultEntityProjectionError::MissingIdentity {
                    kind: "bound receiver parameter",
                    index: bound.receiver_parameter.identity_raw(),
                },
            )?;
        let source = match bound.source {
            crate::BoundCallableSource::Class { bound, callable } => {
                let application = arena_get(&self.export.class_applications, bound)
                    .ok_or_else(|| unknown("bound class application", bound))?;
                DefaultBoundCallableSourceV1::Class {
                    bound: self.type_key(application.canonical_type, binders)?,
                    callable: self.callable(callable, binders)?,
                }
            }
            crate::BoundCallableSource::Interface { bound, member } => {
                let application = arena_get(&self.export.interface_applications, bound)
                    .ok_or_else(|| unknown("bound interface application", bound))?;
                let member = arena_get(&self.export.interface_methods, member)
                    .ok_or_else(|| unknown("interface method", member))?;
                DefaultBoundCallableSourceV1::Interface {
                    bound: self.type_key(application.canonical_type, binders)?,
                    member: self.source_callable_declaration(member.function)?,
                }
            }
        };
        let signature = arena_get(&self.export.function_types, bound.instantiated_signature)
            .ok_or_else(|| unknown("bound callable signature", bound.instantiated_signature))?;
        Ok(DefaultBoundCallableRefV1::new(
            DefaultBinderRefV1::new(receiver.depth, receiver.index),
            source,
            self.type_key(signature.canonical_type, binders)?,
        ))
    }

    fn method_owner_type(
        &self,
        owner: MethodOwnerApplication,
    ) -> Result<TypeId, super::super::DefaultEntityProjectionError> {
        match owner {
            MethodOwnerApplication::Class(id) => arena_get(&self.export.class_applications, id)
                .map(|application| application.canonical_type)
                .ok_or_else(|| unknown("class application", id)),
            MethodOwnerApplication::Struct(id) => arena_get(&self.export.struct_applications, id)
                .map(|application| application.canonical_type)
                .ok_or_else(|| unknown("struct application", id)),
            MethodOwnerApplication::Enum(id) => arena_get(&self.export.enum_applications, id)
                .map(|application| application.canonical_type)
                .ok_or_else(|| unknown("enum application", id)),
            MethodOwnerApplication::Interface(id) => {
                arena_get(&self.export.interface_applications, id)
                    .map(|application| application.canonical_type)
                    .ok_or_else(|| unknown("interface application", id))
            }
            MethodOwnerApplication::Object(id) => arena_get(&self.export.object_types, id)
                .map(|object| object.canonical_type)
                .ok_or_else(|| unknown("object type", id)),
        }
    }

    fn generic_method_owner_type(
        &self,
        owner: GenericMethodOwner,
    ) -> Result<TypeId, super::super::DefaultEntityProjectionError> {
        match owner {
            GenericMethodOwner::Class(id) => arena_get(&self.export.class_applications, id)
                .map(|application| application.canonical_type)
                .ok_or_else(|| unknown("class application", id)),
            GenericMethodOwner::Struct(id) => arena_get(&self.export.struct_applications, id)
                .map(|application| application.canonical_type)
                .ok_or_else(|| unknown("struct application", id)),
            GenericMethodOwner::Enum(id) => arena_get(&self.export.enum_applications, id)
                .map(|application| application.canonical_type)
                .ok_or_else(|| unknown("enum application", id)),
            GenericMethodOwner::Object(id) => arena_get(&self.export.object_types, id)
                .map(|object| object.canonical_type)
                .ok_or_else(|| unknown("object type", id)),
        }
    }
}
