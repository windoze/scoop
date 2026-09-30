use super::*;

impl Lowerer {
    pub(super) fn intrinsic_type_access_domain(
        &self,
        kind: hir::IntrinsicTypeKind,
    ) -> Option<hir::AccessDomain> {
        let &(owner, _) = self.intrinsic_type_owners.get(&kind)?;
        Some(match owner {
            crate::IntrinsicTypeOwner::Struct(owner) => self.structs[owner].access.lookup.0.clone(),
            crate::IntrinsicTypeOwner::Class(owner) => self.classes[owner].access.lookup.0.clone(),
        })
    }

    pub(super) fn collect_type_dependencies(
        &self,
        ty: hir::TypeId,
        dependencies: &mut Vec<(hir::TypeId, hir::AccessDomain)>,
    ) {
        let provided = match self.types[ty] {
            hir::Type::ImportedStruct(ref structure) => {
                Some(self.imported_nominal_access_domain(&structure.declaration))
            }
            hir::Type::ImportedEnum(ref structure) => {
                Some(self.imported_nominal_access_domain(&structure.declaration))
            }
            hir::Type::ImportedClass(ref structure) => {
                Some(self.imported_nominal_access_domain(&structure.declaration))
            }
            hir::Type::ImportedInterface(ref structure) => {
                Some(self.imported_nominal_access_domain(&structure.declaration))
            }
            hir::Type::Integer(kind) => {
                self.intrinsic_type_access_domain(hir::IntrinsicTypeKind::Integer(kind))
            }
            hir::Type::Boolean => {
                self.intrinsic_type_access_domain(hir::IntrinsicTypeKind::Boolean)
            }
            hir::Type::String => self.intrinsic_type_access_domain(hir::IntrinsicTypeKind::String),
            hir::Type::Struct(application) => Some(
                self.structs[self.struct_applications[application].template]
                    .access
                    .lookup
                    .0
                    .clone(),
            ),
            hir::Type::Enum(application) => Some(
                self.enums[self.enum_applications[application].template]
                    .access
                    .lookup
                    .0
                    .clone(),
            ),
            hir::Type::Class(application) => Some(
                self.classes[self.class_applications[application].template]
                    .access
                    .lookup
                    .0
                    .clone(),
            ),
            hir::Type::Interface(application) => Some(
                self.interfaces[self.interface_applications[application].template]
                    .access
                    .lookup
                    .0
                    .clone(),
            ),
            hir::Type::Ptr(_) => self
                .ffi_ptr
                .map(|owner| self.structs[owner].access.lookup.0.clone()),
            hir::Type::FunPtr(_) => self
                .ffi_fun_ptr
                .map(|owner| self.structs[owner].access.lookup.0.clone()),
            hir::Type::Unit
            | hir::Type::Any
            | hir::Type::Tuple(_)
            | hir::Type::Function(_)
            | hir::Type::Param(_) => None,
        };
        if let Some(provided) = provided
            && !dependencies.iter().any(|(dependency, _)| *dependency == ty)
        {
            dependencies.push((ty, provided));
        }

        match self.types[ty].clone() {
            hir::Type::Struct(application) => {
                for argument in self.struct_applications[application].arguments.clone() {
                    self.collect_type_dependencies(argument, dependencies);
                }
            }
            hir::Type::Enum(application) => {
                for argument in self.enum_applications[application].arguments.clone() {
                    self.collect_type_dependencies(argument, dependencies);
                }
            }
            hir::Type::Class(application) => {
                for argument in self.class_applications[application].arguments.clone() {
                    self.collect_type_dependencies(argument, dependencies);
                }
            }
            hir::Type::Interface(application) => {
                for argument in self.interface_applications[application].arguments.clone() {
                    self.collect_type_dependencies(argument, dependencies);
                }
            }
            hir::Type::Tuple(elements) => {
                for element in elements {
                    self.collect_type_dependencies(element, dependencies);
                }
            }
            hir::Type::Function(function) | hir::Type::FunPtr(function) => {
                let function = self.function_types[function].clone();
                for parameter in function.parameter_types {
                    self.collect_type_dependencies(parameter, dependencies);
                }
                self.collect_type_dependencies(function.return_type, dependencies);
            }
            hir::Type::Ptr(pointee) => self.collect_type_dependencies(pointee, dependencies),
            hir::Type::ImportedStruct(_)
            | hir::Type::ImportedEnum(_)
            | hir::Type::ImportedClass(_)
            | hir::Type::ImportedInterface(_)
            | hir::Type::Unit
            | hir::Type::Integer(_)
            | hir::Type::Boolean
            | hir::Type::String
            | hir::Type::Any
            | hir::Type::Param(_) => {}
        }
    }

    pub(crate) fn type_access_domain(&self, ty: hir::TypeId) -> hir::AccessDomain {
        let mut dependencies = Vec::new();
        self.collect_type_dependencies(ty, &mut dependencies);
        dependencies
            .into_iter()
            .fold(hir::AccessDomain::universal(), |domain, (_, dependency)| {
                domain.intersect(&dependency)
            })
    }

    pub(crate) fn default_call_domain(&self, owner: hir::ExportParameterOwner) -> hir::CallDomain {
        let access = match owner {
            hir::ExportParameterOwner::Function(function) => &self.functions[function].access,
            hir::ExportParameterOwner::StructConstructor(constructor) => {
                &self.struct_constructors[constructor].access
            }
            hir::ExportParameterOwner::ClassConstructor(constructor) => {
                &self.class_constructors[constructor].access
            }
            hir::ExportParameterOwner::VariantConstructor(variant) => {
                return hir::CallDomain {
                    direct: self.enums[variant.enumeration()].access.lookup.clone(),
                    slot: None,
                };
            }
        };
        hir::CallDomain {
            direct: access.lookup.clone(),
            slot: access.slot.clone(),
        }
    }

    pub(crate) fn function_access_domain(&self, function: hir::FunctionId) -> hir::AccessDomain {
        self.functions[function].access.lookup.0.clone()
    }

    pub(crate) fn constructor_access_domain(
        &self,
        target: hir::ExportDefaultConstructorTarget,
    ) -> hir::AccessDomain {
        match target {
            hir::ExportDefaultConstructorTarget::Imported { .. } => hir::AccessDomain::universal(),
            hir::ExportDefaultConstructorTarget::Struct(application) => {
                let constructor = self.struct_constructor_applications[application].constructor;
                self.struct_constructors[constructor]
                    .access
                    .lookup
                    .0
                    .clone()
            }
            hir::ExportDefaultConstructorTarget::Class(application) => {
                let constructor = self.class_constructor_applications[application].constructor;
                self.class_constructors[constructor].access.lookup.0.clone()
            }
            hir::ExportDefaultConstructorTarget::Variant(variant) => {
                self.type_access_domain(variant.owner)
            }
        }
    }

    pub(crate) fn field_access_domain(&self, target: hir::FieldRef) -> hir::AccessDomain {
        match target {
            hir::FieldRef::ClassField { owner, field } => match self.types[owner] {
                hir::Type::Class(_) => {
                    let field = self
                        .field_identity_builder
                        .class_declaration(field)
                        .expect("a current class field has its declaration identity");
                    self.properties[self.class_fields[field].property]
                        .access
                        .lookup
                        .0
                        .clone()
                }
                hir::Type::ImportedClass(_) => hir::AccessDomain::universal(),
                _ => unreachable!("a class field retains its declaring class"),
            },
            hir::FieldRef::StructField { owner, .. } => match self.types[owner] {
                hir::Type::Struct(application) => self.structs
                    [self.struct_applications[application].template]
                    .access
                    .lookup
                    .0
                    .clone(),
                hir::Type::ImportedStruct(_) => hir::AccessDomain::universal(),
                _ => unreachable!("a struct field retains its declaring struct"),
            },
            hir::FieldRef::TupleIndex(_) => hir::AccessDomain::universal(),
        }
    }
}
