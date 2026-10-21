//! Source-visible generated members retain their actual callable identities.

use super::*;

impl Lowerer {
    pub(super) fn documentation_function_target(&self, function: hir::FunctionId) -> Target {
        if matches!(
            self.functions[function].kind,
            hir::FunctionKind::DerivedEquality
        ) {
            Target::DerivedFunction(function)
        } else {
            Source::Callable(Callable::Function(function)).into()
        }
    }

    pub(super) fn document_synthesized_members(&mut self) {
        let Some(context) = &self.documentation else {
            return;
        };
        let previous = (
            self.current_file,
            self.current_owner,
            self.type_params_in_scope.clone(),
        );
        let mut pending = self
            .functions
            .iter()
            .filter_map(|(id, function)| {
                let owner = self.function_owner.get(&id).copied()?;
                if self.local_type_root(owner).is_some()
                    || self.local_function_by_function.contains_key(&id)
                    || context
                        .targets
                        .contains(&self.documentation_function_target(id))
                {
                    return None;
                }
                let name = if matches!(function.kind, hir::FunctionKind::DerivedEquality) {
                    "equals"
                } else {
                    &self.source_function_declarations.get(&id)?.name
                };
                Some((id, owner, name.to_owned()))
            })
            .collect::<Vec<_>>();
        pending.sort_by_key(|(id, _, _)| id.into_raw().into_u32());
        for (id, owner, name) in pending {
            self.current_file = self.function_files[&id];
            self.current_owner = Some(owner);
            let signature = self.signatures[&id].clone();
            self.type_params_in_scope = signature.type_params.clone();
            let mut parts = Vec::new();
            if signature.is_suspend {
                parts.push(Part::Text("suspend ".into()));
            }
            if signature.modifiers.operator.is_some()
                || signature.modifiers.property_delegate_operator.is_some()
            {
                parts.push(Part::Text("operator ".into()));
            }
            if signature.modifiers.is_infix {
                parts.push(Part::Text("infix ".into()));
            }
            parts.push(Part::Text(format!("fun {name}(")));
            for (index, parameter) in signature.params.iter().enumerate() {
                if index != 0 {
                    parts.push(Part::Text(", ".into()));
                }
                parts.push(Part::Text(format!("{}: ", parameter.name.text)));
                parts.push(Part::Type(parameter.ty));
            }
            parts.push(Part::Text("): ".into()));
            parts.push(Part::Type(signature.return_ty));
            self.document_declaration(
                self.documentation_function_target(id),
                Self::documentation_owner(Some(owner)),
                Kind::Function,
                &name,
                self.functions[id].span,
                self.functions[id].access.declared,
                parts,
                false,
            );
            let target = self.documentation_function_target(id);
            let declaration = self
                .documentation
                .as_mut()
                .expect("documentation is requested")
                .declarations
                .iter_mut()
                .find(|entry| entry.declaration.target == target)
                .expect("the non-local generated declaration was captured");
            declaration.declaration.synthesized = true;
        }
        (
            self.current_file,
            self.current_owner,
            self.type_params_in_scope,
        ) = previous;
    }
}
