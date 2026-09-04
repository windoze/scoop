use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NominalTemplate {
    Struct(hir::StructId),
    Class(hir::ClassId),
    Enum(hir::EnumId),
    Interface(hir::InterfaceId),
}

#[derive(Debug, Clone)]
struct NominalApplication {
    ty: TypeId,
    template: NominalTemplate,
    arguments: Vec<TypeId>,
}

impl Lowerer {
    /// Add the source-level explanation for the otherwise easy-to-misread
    /// `G<S>` versus `G<T>` mismatch. The source application may be reached
    /// through an ordinary class/interface supertype (for example
    /// `Derived : Base<String>` checked against `Base<Any>`).
    pub(crate) fn with_nominal_invariance_detail(
        &mut self,
        message: String,
        found: TypeId,
        expected: TypeId,
    ) -> String {
        let Some(detail) = self.nominal_invariance_detail(found, expected) else {
            return message;
        };
        format!("{message}; {detail}")
    }

    fn nominal_invariance_detail(&mut self, found: TypeId, expected: TypeId) -> Option<String> {
        let target = self.nominal_application(expected)?;
        let mut pending = vec![found];
        let mut seen = Vec::new();
        while let Some(ty) = pending.pop() {
            if seen.iter().any(|&visited| self.types_equal(visited, ty)) {
                continue;
            }
            seen.push(ty);
            if let Some(source) = self.nominal_application(ty)
                && source.template == target.template
                && source.arguments.len() == target.arguments.len()
                && let Some(index) = source
                    .arguments
                    .iter()
                    .zip(&target.arguments)
                    .position(|(&source, &target)| !self.types_equal(source, target))
            {
                let parameter = self.nominal_parameter_name(target.template, index);
                let source_application = self.type_name(source.ty);
                let target_application = self.type_name(target.ty);
                let source_argument = self.type_name(source.arguments[index]);
                let target_argument = self.type_name(target.arguments[index]);
                return Some(format!(
                    "nominal application `{}` is invariant: first differing type argument {} (`{parameter}`) is `{source_argument}` in `{source_application}`, but `{target_argument}` in `{target_application}`; construct the target application directly or convert explicitly",
                    self.nominal_template_name(target.template),
                    index + 1,
                ));
            }
            pending.extend(self.direct_nominal_supertypes(ty));
        }
        None
    }

    fn nominal_application(&self, ty: TypeId) -> Option<NominalApplication> {
        let (template, arguments) = match self.types[ty] {
            Type::Struct(application) => {
                let application = &self.struct_applications[application];
                (
                    NominalTemplate::Struct(application.template),
                    application.arguments.clone(),
                )
            }
            Type::Class(application) => {
                let application = &self.class_applications[application];
                (
                    NominalTemplate::Class(application.template),
                    application.arguments.clone(),
                )
            }
            Type::Enum(application) => {
                let application = &self.enum_applications[application];
                (
                    NominalTemplate::Enum(application.template),
                    application.arguments.clone(),
                )
            }
            Type::Interface(application) => {
                let application = &self.interface_applications[application];
                (
                    NominalTemplate::Interface(application.template),
                    application.arguments.clone(),
                )
            }
            _ => return None,
        };
        Some(NominalApplication {
            ty,
            template,
            arguments,
        })
    }

    fn nominal_template_name(&self, template: NominalTemplate) -> &str {
        match template {
            NominalTemplate::Struct(id) => &self.structs[id].name,
            NominalTemplate::Class(id) => &self.classes[id].name,
            NominalTemplate::Enum(id) => &self.enums[id].name,
            NominalTemplate::Interface(id) => &self.interfaces[id].name,
        }
    }

    fn nominal_parameter_name(&self, template: NominalTemplate, index: usize) -> &str {
        match template {
            NominalTemplate::Struct(id) => &self.structs[id].type_params[index].name,
            NominalTemplate::Class(id) => &self.classes[id].type_params[index].name,
            NominalTemplate::Enum(id) => &self.enums[id].type_params[index].name,
            NominalTemplate::Interface(id) => &self.interfaces[id].type_params[index].name,
        }
    }

    fn direct_nominal_supertypes(&mut self, ty: TypeId) -> Vec<TypeId> {
        match self.types[ty].clone() {
            Type::Class(application) => {
                let application = self.class_applications[application].clone();
                let declaration = self.classes[application.template].clone();
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
                let interfaces = self.structs[application.template].interfaces.clone();
                interfaces
                    .into_iter()
                    .map(|interface| self.instantiate_ty(interface, &application.arguments))
                    .collect()
            }
            Type::Enum(application) => {
                let application = self.enum_applications[application].clone();
                let interfaces = self.enums[application.template].interfaces.clone();
                interfaces
                    .into_iter()
                    .map(|interface| self.instantiate_ty(interface, &application.arguments))
                    .collect()
            }
            Type::Interface(application) => {
                let application = self.interface_applications[application].clone();
                let parents = self.interfaces[application.template].parents.clone();
                parents
                    .into_iter()
                    .map(|parent| {
                        let parent = self.interface_applications[parent].canonical_type;
                        self.instantiate_ty(parent, &application.arguments)
                    })
                    .collect()
            }
            Type::Param(parameter) => self
                .type_params_in_scope
                .iter()
                .find(|candidate| candidate.id == parameter)
                .map(hir::TypeParamDecl::nominal_bounds_in_source_order)
                .unwrap_or_default()
                .into_iter()
                .map(|bound| match bound {
                    hir::NominalBoundRef::Class(bound) => {
                        self.class_applications[bound.application].canonical_type
                    }
                    hir::NominalBoundRef::Interface(bound) => {
                        self.interface_applications[bound.application].canonical_type
                    }
                })
                .collect(),
            Type::Int => self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::Int),
            Type::UInt => self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::UInt),
            Type::Boolean => self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::Boolean),
            Type::String => self.intrinsic_type_interfaces(hir::IntrinsicTypeKind::String),
            Type::Unit
            | Type::Any
            | Type::Tuple(_)
            | Type::Function(_)
            | Type::Ptr(_)
            | Type::FunPtr(_) => Vec::new(),
        }
    }
}
