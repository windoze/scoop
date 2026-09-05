//! Structural type-property classification shared by effect validation.

use std::collections::HashSet;

use scoop_ast::Span;
use scoop_hir as hir;

use crate::Lowerer;

/// Type arguments in a generic aggregate are expressed in the caller's
/// parameter namespace. Chaining environments lets structural classification
/// substitute nested generic fields without interning synthetic HIR types.
struct TypeEnvironment<'a> {
    bindings: Vec<(hir::TypeParamId, hir::TypeId)>,
    parent: Option<&'a TypeEnvironment<'a>>,
}

impl<'a> TypeEnvironment<'a> {
    fn resolve(
        &'a self,
        parameter: hir::TypeParamId,
    ) -> Option<(hir::TypeId, Option<&'a TypeEnvironment<'a>>)> {
        if let Some(argument) = self
            .bindings
            .iter()
            .find_map(|(candidate, argument)| (*candidate == parameter).then_some(*argument))
        {
            return Some((argument, self.parent));
        }
        self.parent?.resolve(parameter)
    }
}

impl Lowerer {
    pub(crate) fn is_gc_free(&self, ty: hir::TypeId) -> bool {
        self.gc_free_requirements(ty)
            .is_some_and(|requirements| requirements.is_empty())
    }

    pub(super) fn gc_free_requirements(
        &self,
        ty: hir::TypeId,
    ) -> Option<HashSet<hir::TypeParamId>> {
        self.gc_free_requirements_inner(ty, None, &mut HashSet::new())
    }

    fn gc_free_requirements_inner(
        &self,
        ty: hir::TypeId,
        environment: Option<&TypeEnvironment<'_>>,
        visiting: &mut HashSet<hir::TypeId>,
    ) -> Option<HashSet<hir::TypeParamId>> {
        match &self.types[ty] {
            hir::Type::Unit
            | hir::Type::Integer(_)
            | hir::Type::Boolean
            | hir::Type::Ptr(_)
            | hir::Type::FunPtr(_) => Some(HashSet::new()),
            hir::Type::String
            | hir::Type::Class(..)
            | hir::Type::Interface(_)
            | hir::Type::Any
            | hir::Type::Function(_) => None,
            hir::Type::Tuple(elements) => {
                let mut requirements = HashSet::new();
                for element in elements {
                    requirements.extend(self.gc_free_requirements_inner(
                        *element,
                        environment,
                        visiting,
                    )?);
                }
                Some(requirements)
            }
            hir::Type::Param(index) => match environment {
                Some(environment) => {
                    let (argument, parent) = environment
                        .resolve(*index)
                        .expect("the complete type environment binds every referenced parameter");
                    self.gc_free_requirements_inner(argument, parent, visiting)
                }
                None => Some(HashSet::from([*index])),
            },
            hir::Type::Struct(application) => {
                let application = &self.struct_applications[*application];
                let id = application.template;
                if !visiting.insert(ty) {
                    return None;
                }
                let nested = TypeEnvironment {
                    bindings: self.structs[id]
                        .type_params
                        .iter()
                        .zip(application.arguments.iter().copied())
                        .map(|(parameter, argument)| (parameter.id, argument))
                        .collect(),
                    parent: environment,
                };
                let mut requirements = HashSet::new();
                for field in self.structs[id].semantic_fields() {
                    let Some(required) =
                        self.gc_free_requirements_inner(field.ty, Some(&nested), visiting)
                    else {
                        visiting.remove(&ty);
                        return None;
                    };
                    requirements.extend(required);
                }
                visiting.remove(&ty);
                Some(requirements)
            }
            hir::Type::Enum(application) => {
                let application = &self.enum_applications[*application];
                let id = application.template;
                if !visiting.insert(ty) {
                    return None;
                }
                let nested = TypeEnvironment {
                    bindings: self.enums[id]
                        .type_params
                        .iter()
                        .zip(application.arguments.iter().copied())
                        .map(|(parameter, argument)| (parameter.id, argument))
                        .collect(),
                    parent: environment,
                };
                let mut requirements = HashSet::new();
                for field in self.enums[id]
                    .variants
                    .iter()
                    .flat_map(|variant| &variant.fields)
                {
                    let Some(required) =
                        self.gc_free_requirements_inner(field.ty, Some(&nested), visiting)
                    else {
                        visiting.remove(&ty);
                        return None;
                    };
                    requirements.extend(required);
                }
                visiting.remove(&ty);
                Some(requirements)
            }
        }
    }

    pub(crate) fn requires_unsafe_use(&self, ty: hir::TypeId) -> bool {
        self.requires_unsafe_use_inner(ty, None, &mut HashSet::new())
    }

    fn requires_unsafe_use_inner(
        &self,
        ty: hir::TypeId,
        environment: Option<&TypeEnvironment<'_>>,
        visiting: &mut HashSet<hir::TypeId>,
    ) -> bool {
        match &self.types[ty] {
            hir::Type::Struct(application) => {
                let application = &self.struct_applications[*application];
                let id = application.template;
                if self.structs[id].attributes.interior_mutable {
                    return true;
                }
                if !visiting.insert(ty) {
                    return false;
                }
                let nested = TypeEnvironment {
                    bindings: self.structs[id]
                        .type_params
                        .iter()
                        .zip(application.arguments.iter().copied())
                        .map(|(parameter, argument)| (parameter.id, argument))
                        .collect(),
                    parent: environment,
                };
                let result = self.structs[id]
                    .semantic_fields()
                    .iter()
                    .any(|field| self.requires_unsafe_use_inner(field.ty, Some(&nested), visiting));
                visiting.remove(&ty);
                result
            }
            hir::Type::Enum(application) => {
                let application = &self.enum_applications[*application];
                let id = application.template;
                if !visiting.insert(ty) {
                    return false;
                }
                let nested = TypeEnvironment {
                    bindings: self.enums[id]
                        .type_params
                        .iter()
                        .zip(application.arguments.iter().copied())
                        .map(|(parameter, argument)| (parameter.id, argument))
                        .collect(),
                    parent: environment,
                };
                let result = self.enums[id].variants.iter().any(|variant| {
                    variant.fields.iter().any(|field| {
                        self.requires_unsafe_use_inner(field.ty, Some(&nested), visiting)
                    })
                });
                visiting.remove(&ty);
                result
            }
            hir::Type::Tuple(elements) => elements
                .iter()
                .any(|ty| self.requires_unsafe_use_inner(*ty, environment, visiting)),
            hir::Type::Param(index) => environment.is_some_and(|environment| {
                let (argument, parent) = environment
                    .resolve(*index)
                    .expect("the complete type environment binds every referenced parameter");
                self.requires_unsafe_use_inner(argument, parent, visiting)
            }),
            _ => false,
        }
    }

    pub(crate) fn require_unsafe_type_use(&mut self, ty: hir::TypeId, span: Span) -> bool {
        if !self.requires_unsafe_use(ty) {
            return true;
        }
        let safety = self
            .safety_contexts
            .last()
            .copied()
            .expect("the safety context stack is initialized");
        if safety == hir::Safety::Unsafe {
            return true;
        }
        self.error(
            span,
            format!(
                "value of type {} contains `@InteriorMutable` state and requires an unsafe context",
                self.type_name(ty)
            ),
        );
        false
    }
}
