//! Candidate collection by the complete receiver type.

use super::*;

impl Lowerer {
    pub(super) fn method_candidates(
        &mut self,
        ty: TypeId,
    ) -> Vec<(crate::CallableCandidate, usize, usize)> {
        let mut declared = Vec::<(crate::CallableCandidate, usize, usize)>::new();
        match self.types[ty].clone() {
            Type::Integer(kind) => self.collect_intrinsic_type_methods(
                hir::IntrinsicTypeKind::Integer(kind),
                &mut declared,
            ),
            Type::Unit => {
                self.collect_intrinsic_type_methods(hir::IntrinsicTypeKind::Unit, &mut declared)
            }
            Type::Boolean => {
                self.collect_intrinsic_type_methods(hir::IntrinsicTypeKind::Boolean, &mut declared)
            }
            Type::String => {
                self.collect_intrinsic_type_methods(hir::IntrinsicTypeKind::String, &mut declared)
            }
            Type::Class(application) => {
                self.collect_class_method_candidates(application, 0, None, &mut declared)
            }
            Type::Interface(application) => {
                self.collect_interface_method_candidates(
                    application,
                    0,
                    0,
                    None,
                    &mut Vec::new(),
                    &mut declared,
                );
            }
            Type::Struct(application) => {
                let application_value = self.struct_applications[application].clone();
                declared.extend(
                    self.source_struct_id(application_value.template)
                        .into_iter()
                        .flat_map(|id| self.structs[id].methods.iter().copied())
                        .map(|function| {
                            (
                                crate::CallableCandidate::method(
                                    function,
                                    hir::MethodOwnerApplication::Struct(application),
                                ),
                                0,
                                0,
                            )
                        }),
                );
            }
            Type::Ptr(pointee) => {
                if let Some(owner) = self.ffi_ptr {
                    let application = self.struct_application_id(owner, vec![pointee]);
                    declared.extend(self.structs[owner].methods.iter().copied().map(|function| {
                        (
                            crate::CallableCandidate::method(
                                function,
                                hir::MethodOwnerApplication::Struct(application),
                            ),
                            0,
                            0,
                        )
                    }));
                }
            }
            Type::FunPtr(_) => {}
            Type::Enum(application) => {
                let application_value = self.enum_applications[application].clone();
                declared.extend(
                    self.source_enum_id(application_value.template)
                        .into_iter()
                        .flat_map(|id| self.enums[id].methods.iter().copied())
                        .map(|function| {
                            (
                                crate::CallableCandidate::method(
                                    function,
                                    hir::MethodOwnerApplication::Enum(application),
                                ),
                                0,
                                0,
                            )
                        }),
                );
            }
            Type::Any => {}
            Type::Param(receiver_parameter) => {
                let parameter = self
                    .type_params_in_scope
                    .iter()
                    .find(|parameter| parameter.id == receiver_parameter)
                    .expect("the receiver parameter is in the active declaration scope")
                    .clone();
                for (root, bound) in parameter
                    .nominal_bounds_in_source_order()
                    .into_iter()
                    .enumerate()
                {
                    match self.types[bound.ty()] {
                        Type::Class(application) => {
                            self.collect_class_method_candidates(
                                application,
                                root,
                                Some((receiver_parameter, application)),
                                &mut declared,
                            );
                        }
                        Type::Interface(application) => {
                            self.collect_interface_method_candidates(
                                application,
                                0,
                                root,
                                Some((receiver_parameter, application)),
                                &mut Vec::new(),
                                &mut declared,
                            );
                        }
                        _ => unreachable!("nominal bounds retain a class or interface type"),
                    }
                }
            }
            _ => {}
        }

        self.collect_selected_interface_members(ty, &mut declared);
        declared
    }
}
