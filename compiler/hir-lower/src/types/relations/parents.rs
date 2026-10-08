//! Declaration storage adaptation for the shared parent-type queries.

use super::*;

impl Lowerer {
    pub(crate) fn direct_nominal_supertypes(&mut self, ty: TypeId) -> Vec<TypeId> {
        self.direct_nominal_supertypes_for(self.types[ty].clone())
    }

    pub(super) fn direct_nominal_supertypes_for(&mut self, ty: Type) -> Vec<TypeId> {
        match ty {
            Type::Class(application) => {
                let application = self.class_applications[application].clone();
                let declaration = self.class_definition(application.template).clone();
                let mut result = declaration
                    .base_class
                    .map(|base| self.instantiate_ty(base, &application.arguments))
                    .into_iter()
                    .collect::<Vec<_>>();
                result.extend(
                    declaration
                        .interfaces
                        .into_iter()
                        .map(|interface| self.instantiate_ty(interface, &application.arguments)),
                );
                result
            }
            Type::Struct(application) => {
                let application = self.struct_applications[application].clone();
                let interfaces = self
                    .struct_definition(application.template)
                    .interfaces
                    .clone();
                interfaces
                    .into_iter()
                    .map(|interface| self.instantiate_ty(interface, &application.arguments))
                    .collect()
            }
            Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                let interfaces = self
                    .enum_definition(application.template)
                    .interfaces
                    .clone();
                interfaces
                    .into_iter()
                    .map(|interface| self.instantiate_ty(interface, &application.arguments))
                    .collect()
            }
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                let parents = self
                    .interface_definition(application.template)
                    .parents
                    .clone();
                parents
                    .into_iter()
                    .map(|parent| self.instantiate_ty(parent, &application.arguments))
                    .collect()
            }
            Type::Param(parameter) => self
                .type_params_in_scope
                .iter()
                .find(|candidate| candidate.id == parameter)
                .map(hir::TypeParamDecl::nominal_bounds_in_source_order)
                .unwrap_or_default()
                .into_iter()
                .map(|bound| bound.ty())
                .collect(),
            Type::Integer(kind) => {
                self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::Integer(kind))
            }
            Type::Unit => self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::Unit),
            Type::Boolean => self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::Boolean),
            Type::String => self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::String),
            Type::Ptr(pointee) => self.pointer_interfaces(pointee),
            Type::Any | Type::Tuple(_) | Type::Function(_) | Type::FunPtr(_) => Vec::new(),
        }
    }

    /// Interfaces explicitly declared by the source definition of one
    /// compiler-represented intrinsic type. Primitive `Type` variants retain
    /// their local owner or the declaration resolved from ordinary dependency
    /// metadata; the interface set is not built into the compiler.
    pub(crate) fn intrinsic_type_interfaces(
        &mut self,
        kind: hir::IntrinsicTypeKind,
    ) -> Vec<TypeId> {
        if let Err(error) = self.resolve_imported_intrinsic_type(kind) {
            self.diagnostics.push(ast::Diagnostic::without_span(
                ast::DiagnosticSeverity::Error,
                self.current_file,
                error.diagnostic(&format!(
                    "interfaces of intrinsic type `{}`",
                    kind.source_name()
                )),
            ));
            return Vec::new();
        }
        if let Some(source) = self.imported_intrinsic_types.get(&kind) {
            return source.interfaces.clone();
        }
        let Some(&(owner, _provider)) = self.intrinsic_type_owners.get(&kind) else {
            // A missing intrinsic owner is diagnosed by the core-contract
            // validator and prevents HIR output.  During recovery there is no
            // source declaration whose interfaces could be consumed.
            return Vec::new();
        };
        match owner {
            crate::IntrinsicTypeOwner::Struct(owner) => self.structs[owner].interfaces.clone(),
            crate::IntrinsicTypeOwner::Class(owner) => {
                self.class_interfaces_for_application(self.classes[owner].self_application)
            }
        }
    }
}
