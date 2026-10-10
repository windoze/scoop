//! Struct shells and their concrete declared fields.

use super::*;

impl Lowerer {
    pub(crate) fn lower_structs(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.structs.iter() {
            let representation = match &decl.representation {
                hir::StructRepresentation::Declared(hir::DeclaredStruct {
                    attributes,
                    c_abi,
                    ..
                }) => mir::StructRepresentation::Declared(mir::DeclaredStruct {
                    c_abi: match c_abi {
                        hir::StructCAbi::SourceRepresentation => {
                            mir::StructCAbi::SourceRepresentation
                        }
                        hir::StructCAbi::UInt64Field { field } => {
                            mir::StructCAbi::UInt64Field { field: *field }
                        }
                    },
                    c_layout: attributes.c_layout.map(|layout| mir::MirCLayoutContract {
                        aligned: lower_c_layout_value(layout.aligned),
                        packed: lower_c_layout_value(layout.packed),
                    }),
                    interior_mutable: attributes.interior_mutable,
                    fields: Vec::new(),
                }),
                hir::StructRepresentation::Intrinsic { application, .. } => {
                    mir::StructRepresentation::Intrinsic(match application {
                        hir::IntrinsicTypeRepresentation::MaybeUninit { value } => {
                            let types = Types {
                                module,
                                struct_map: &self.struct_map,
                                class_map: &self.class_map,
                            };
                            mir::IntrinsicStructRepresentation::MaybeUninit {
                                value: types.lower(
                                    *value,
                                    &mut self.source_exact_types,
                                    &mut self.enums,
                                    &mut self.structs,
                                    &mut self.interfaces,
                                    &mut self.shell,
                                ),
                            }
                        }
                        hir::IntrinsicTypeRepresentation::Unit => {
                            mir::IntrinsicStructRepresentation::Unit
                        }
                        hir::IntrinsicTypeRepresentation::Integer(kind) => {
                            mir::IntrinsicStructRepresentation::Integer(lower_integer_kind(*kind))
                        }
                        hir::IntrinsicTypeRepresentation::Float(kind) => {
                            mir::IntrinsicStructRepresentation::Float(*kind)
                        }
                        hir::IntrinsicTypeRepresentation::Char => {
                            mir::IntrinsicStructRepresentation::Char
                        }
                        hir::IntrinsicTypeRepresentation::Boolean => {
                            mir::IntrinsicStructRepresentation::Boolean
                        }
                        hir::IntrinsicTypeRepresentation::Ptr { pointee } => {
                            let types = Types {
                                module,
                                struct_map: &self.struct_map,
                                class_map: &self.class_map,
                            };
                            mir::IntrinsicStructRepresentation::Ptr {
                                pointee: types.lower(
                                    *pointee,
                                    &mut self.source_exact_types,
                                    &mut self.enums,
                                    &mut self.structs,
                                    &mut self.interfaces,
                                    &mut self.shell,
                                ),
                            }
                        }
                        hir::IntrinsicTypeRepresentation::FunPtr { signature } => {
                            mir::IntrinsicStructRepresentation::FunPtr {
                                signature: remap_idx(*signature),
                            }
                        }
                        hir::IntrinsicTypeRepresentation::String
                        | hir::IntrinsicTypeRepresentation::Atomic(_)
                        | hir::IntrinsicTypeRepresentation::Any
                        | hir::IntrinsicTypeRepresentation::Nothing
                        | hir::IntrinsicTypeRepresentation::Array { .. }
                        | hir::IntrinsicTypeRepresentation::MutableArray { .. } => {
                            unreachable!("the registry fixes intrinsic declaration targets")
                        }
                    })
                }
            };
            let mir_id = self.structs.defs.alloc(mir::StructDef {
                name: decl.name.clone(),
                type_arguments: Vec::new(),
                gc_free: decl.gc_free,
                representation,
            });
            assert_eq!(mir_id, self.struct_map[&hir_id]);
            self.structs.hir_ids.insert(mir_id, hir_id);
        }
    }

    /// Fill the MIR struct field types. This runs after the type context
    /// exists because field types can reference concrete enums.
    pub(crate) fn fill_struct_fields(&mut self, module: &hir::Module) {
        for (hir_id, decl) in module.structs.iter() {
            let hir::StructRepresentation::Declared(declared) = &decl.representation else {
                continue;
            };
            let types = Types {
                module,
                struct_map: &self.struct_map,
                class_map: &self.class_map,
            };
            let fields = declared
                .fields
                .iter()
                .map(|field| mir::DeclaredStructField {
                    identity: field.identity,
                    name: field.name.clone(),
                    ty: types.lower(
                        field.ty,
                        &mut self.source_exact_types,
                        &mut self.enums,
                        &mut self.structs,
                        &mut self.interfaces,
                        &mut self.shell,
                    ),
                })
                .collect();
            let mir_id = self.struct_map[&hir_id];
            match &mut self.structs.defs[mir_id].representation {
                mir::StructRepresentation::Declared(mir::DeclaredStruct {
                    fields: mir_fields,
                    ..
                }) => *mir_fields = fields,
                mir::StructRepresentation::Intrinsic(_) => {
                    unreachable!("a declared source fills its declared MIR shell")
                }
            }
        }
    }
}
