use super::*;
use std::collections::BTreeSet;

impl Lowerer {
    pub(crate) fn prepare_type_reference_factory(&mut self) -> Result<(), String> {
        let CoreLoweringAuthority::Imported(core) = &self.core else {
            return Ok(());
        };
        let hir::ImportedCoreProtocolCallableDefinition::GenericFunction(factory) =
            core.type_reference().definition()
        else {
            unreachable!("the checked core type reference factory is generic")
        };
        let declaration = self
            .dependencies
            .as_ref()
            .expect("imported protocols have a dependency catalog")
            .callable_declaration(scoop_identity::CallableTemplateOrigin::GenericFunction(
                factory.persistent(),
            ))
            .map_err(|error| error.to_string())?;
        self.request_imported_generic_template(declaration)?;
        Ok(())
    }

    pub(crate) fn prepare_type_info_dependencies(&mut self) -> Result<(), String> {
        let describes = |kind: &hir::FunctionKind| matches!(kind, hir::FunctionKind::Intrinsic(intrinsic) if intrinsic.kind.is_type_info());
        if !self
            .functions
            .iter()
            .any(|(_, function)| describes(&function.kind))
            && !self.imported_generic_templates.has_type_info()
        {
            return Ok(());
        }
        let annotations = self
            .dependencies
            .as_ref()
            .expect("dependency selection is installed")
            .annotation_value_types()
            .cloned()
            .collect::<Vec<_>>();
        for ty in annotations {
            self.imported_signature_type(&ty)
                .map_err(|error| format!("cannot load annotation description type: {error:?}"))?;
        }

        let mut visited = BTreeSet::new();
        loop {
            let owners = self
                .loaded_struct_definitions
                .keys()
                .chain(self.loaded_enum_definitions.keys())
                .chain(self.loaded_class_definitions.keys())
                .chain(self.loaded_interface_definitions.keys())
                .copied()
                .filter(|owner| !visited.contains(owner))
                .collect::<Vec<_>>();
            if owners.is_empty() {
                break;
            }
            for owner in owners {
                visited.insert(owner);
                let dependencies = self.dependencies.as_ref().unwrap();
                let arity = dependencies
                    .nominal_declaration(owner)
                    .unwrap()
                    .interface
                    .type_parameters()
                    .binders()
                    .len();
                let types = dependencies
                    .description_member_types(owner)
                    .cloned()
                    .collect::<Vec<_>>();
                let bindings = (0..arity)
                    .map(|index| {
                        let parameter = self.fresh_type_param(index);
                        (
                            SignatureTypeKey::Binder {
                                depth: 0,
                                index: index as u32,
                            },
                            self.intern_type(hir::Type::Param(parameter)),
                        )
                    })
                    .collect();
                for ty in types {
                    self.imported_signature_type_with_bindings(&ty, &bindings)
                        .map_err(|error| {
                            format!("cannot load member description type: {error:?}")
                        })?;
                }
            }
        }
        Ok(())
    }
}
