//! Implicit storage accessors reuse the property's resolved source type.

use scoop_hir as hir;

use super::{PropertyAccessorKind, PropertyAccessorSource};
use crate::{FnParam, FnParamCalling, FnSig, Lowerer};

impl Lowerer {
    pub(in crate::properties) fn resolve_top_level_storage_accessor_signature(
        &mut self,
        source: &PropertyAccessorSource,
        value_type: hir::TypeId,
        initialization: hir::TopLevelInitialization,
    ) {
        let mut attributes = self.functions[source.function].attributes;
        if initialization == hir::TopLevelInitialization::Image && self.is_gc_free(value_type) {
            attributes.gc_effect = hir::GcEffect::NoGc;
        }
        let capability = self.properties[source.property].capability;
        let (params, return_ty) = match source.kind {
            PropertyAccessorKind::Getter => {
                self.property_getters[capability.getter()].attributes = attributes;
                (Vec::new(), value_type)
            }
            PropertyAccessorKind::Setter => {
                let setter = capability
                    .setter()
                    .expect("a setter has mutable capability");
                self.property_setters[setter].attributes = attributes;
                let [parameter] = source.declaration.params.as_slice() else {
                    unreachable!("an implicit setter has one required value parameter")
                };
                (
                    vec![FnParam {
                        name: parameter.name.clone(),
                        calling: FnParamCalling::Required,
                        ty: value_type,
                    }],
                    self.unit,
                )
            }
        };
        let function = &mut self.functions[source.function];
        function.attributes = attributes;
        function.return_ty = return_ty;
        self.signatures.insert(
            source.function,
            FnSig {
                context_parameters: Vec::new(),
                is_suspend: false,
                modifiers: function.modifiers,
                attributes,
                owner_type_param_count: 0,
                type_params: Vec::new(),
                params,
                return_ty,
            },
        );
    }
}
