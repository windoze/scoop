//! Callable applications, bounded dispatch, and imported-core targets.

use scoop_identity::OptionalSignatureType;

use super::{DefaultEntityProjector, arena_get, unknown};
use crate::{
    Callable, DefaultBoundCallableRefV1, DefaultBoundCallableSourceV1,
    DefaultCallableDeclarationV1, DefaultCallableRefV1, GenericMethodOwner, HirSignatureBinder,
    ImportedDependencyCallableUseId, MethodOwnerApplication, TypeId,
};

impl DefaultEntityProjector<'_> {
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
                (
                    generic.function,
                    None,
                    self.type_arguments(application.type_args.iter(), binders)?,
                )
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
                    self.type_arguments(application.method_arguments.iter(), binders)?,
                )
            }
        };
        let owner = owner.map(|ty| self.type_key(ty, binders)).transpose()?;
        DefaultCallableRefV1::try_new(
            self.callable_declaration(function)?,
            OptionalSignatureType::from_option(owner),
            arguments,
        )
        .map_err(super::super::DefaultEntityProjectionError::Callable)
    }

    fn type_arguments<'a>(
        &self,
        arguments: impl Iterator<Item = &'a TypeId>,
        binders: &[HirSignatureBinder],
    ) -> Result<Vec<scoop_identity::SignatureTypeKey>, super::super::DefaultEntityProjectionError>
    {
        arguments.map(|&ty| self.type_key(ty, binders)).collect()
    }

    pub(in crate::production::default_templates) fn imported_dependency_source(
        &self,
        id: ImportedDependencyCallableUseId,
    ) -> Result<
        &crate::SelectedImportedDependencyCallable,
        super::super::DefaultEntityProjectionError,
    > {
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
        Ok(selected)
    }

    pub(in crate::production::default_templates) fn imported_dependency_callable(
        &self,
        id: ImportedDependencyCallableUseId,
    ) -> Result<DefaultCallableRefV1, super::super::DefaultEntityProjectionError> {
        let interface = self.imported_dependency_source(id)?.interface();
        let source = interface.declaration();
        let declaration = match source {
            scoop_identity::CallableTemplateOrigin::Function(id) => {
                DefaultCallableDeclarationV1::Function(id)
            }
            scoop_identity::CallableTemplateOrigin::GenericFunction(id) => {
                DefaultCallableDeclarationV1::GenericFunction(id)
            }
            scoop_identity::CallableTemplateOrigin::Accessor(id) => {
                DefaultCallableDeclarationV1::PropertyAccessor(id)
            }
            scoop_identity::CallableTemplateOrigin::Constructor(_)
            | scoop_identity::CallableTemplateOrigin::VariantConstructor(_) => {
                return Err(super::super::DefaultEntityProjectionError::CallableKind(
                    source,
                ));
            }
        };
        let owner = if self.export.imported_dependency_callables[id].dispatch()
            == crate::ImportedDependencyDispatch::Direct
            && interface.modality() != crate::CallableModalityV1::Final
        {
            let crate::PublicDeclarationOwnerV1::Nominal(crate::SourceNominalId::Concrete(owner)) =
                interface.owner()
            else {
                return Err(super::super::DefaultEntityProjectionError::CallableKind(
                    source,
                ));
            };
            OptionalSignatureType::Present(Box::new(scoop_identity::SignatureTypeKey::Nominal(
                owner,
            )))
        } else {
            OptionalSignatureType::Absent
        };
        DefaultCallableRefV1::try_new(declaration, owner, Vec::new())
            .map_err(super::super::DefaultEntityProjectionError::Callable)
    }

    pub(in crate::production::default_templates) fn imported_constructor(
        &self,
        declaration: scoop_identity::PersistentConstructorId,
        owner_type: TypeId,
        binders: &[HirSignatureBinder],
    ) -> Result<crate::DefaultConstructorRefV1, super::super::DefaultEntityProjectionError> {
        let source_type = self.type_key(owner_type, binders)?;
        match &self.export.types[owner_type] {
            crate::Type::Struct(_) | crate::Type::ImportedStruct(_) => {
                Ok(crate::DefaultConstructorRefV1::Struct {
                    declaration,
                    owner_type: source_type,
                })
            }
            crate::Type::Class(_) | crate::Type::ImportedClass(_) => {
                Ok(crate::DefaultConstructorRefV1::Class {
                    declaration: crate::DefaultClassConstructorIdV1::Source(declaration),
                    owner_type: source_type,
                })
            }
            _ => Err(super::super::DefaultEntityProjectionError::CallableKind(
                scoop_identity::CallableTemplateOrigin::Constructor(declaration),
            )),
        }
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
            scoop_identity::SignatureTypeKey::Binder {
                depth: receiver.depth,
                index: receiver.index,
            },
            source,
            self.type_key(signature.canonical_type, binders)?,
        ))
    }

    pub(in crate::production::default_templates) fn imported_bound_callable(
        &self,
        bound: &crate::ImportedInterfaceBoundCallable,
        binders: &[HirSignatureBinder],
    ) -> Result<DefaultBoundCallableRefV1, super::super::DefaultEntityProjectionError> {
        let signature = arena_get(&self.export.function_types, bound.signature)
            .ok_or_else(|| unknown("bound callable signature", bound.signature))?;
        Ok(DefaultBoundCallableRefV1::new(
            self.type_key(bound.receiver_type, binders)?,
            DefaultBoundCallableSourceV1::Interface {
                bound: self.type_key(bound.interface, binders)?,
                member: bound.member,
            },
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
