use super::*;

impl Lowerer {
    pub(crate) fn validate_iteration_core(
        &mut self,
        files: &[ast::SourceFile],
    ) -> Option<hir::IterationCore> {
        let iterator = self.require_core_interface("Iterator", files);
        let iterable = self.require_core_interface("Iterable", files);

        let core = iterator.and_then(|iterator| {
            let next = self.interfaces[iterator].methods.as_slice();
            let [next] = next else {
                self.current_file = self.interface_files[&iterator];
                self.error(
                    self.interfaces[iterator].span,
                    "interface `Iterator<T>` in scoop.core must declare exactly `public fun next(): Option<T>`"
                        .to_string(),
                );
                return None;
            };
            let option = self.option_core?;
            let checked = hir::IterationCore::checked(
                &self.interfaces,
                &self.interface_applications,
                &self.interface_method_entities,
                &self.functions,
                &self.enums,
                &self.enum_applications,
                &self.types,
                option,
                iterator,
                *next,
            );
            if checked.is_none() {
                self.current_file = self.interface_files[&iterator];
                self.error(
                    self.interfaces[iterator].span,
                    "interface `Iterator<T>` in scoop.core must declare exactly `public fun next(): Option<T>`"
                        .to_string(),
                );
            }
            checked
        });

        if let (Some(iterable), Some(core)) = (iterable, core) {
            self.validate_iterable_contract(iterable, core);
        }
        core
    }

    fn validate_iterable_contract(&mut self, iterable: InterfaceId, core: hir::IterationCore) {
        self.current_file = self.interface_files[&iterable];
        if !self.iterable_contract_is_valid(iterable, core) {
            let span = self.interfaces[iterable].span;
            self.error(
                span,
                "interface `Iterable<T>` in scoop.core must declare exactly `public operator fun iterator(): Iterator<T>`"
                    .to_string(),
            );
        }
    }

    fn iterable_contract_is_valid(&self, iterable: InterfaceId, core: hir::IterationCore) -> bool {
        let declaration = &self.interfaces[iterable];
        let [parameter] = declaration.type_params.as_slice() else {
            return false;
        };
        let [member] = declaration.methods.as_slice() else {
            return false;
        };
        let relation = &self.interface_method_entities[*member];
        let function = &self.functions[relation.function];
        let self_application = &self.interface_applications[declaration.self_application];
        let Type::Interface(result_id) = self.types[function.return_ty] else {
            return false;
        };
        let result = &self.interface_applications[result_id];
        let hir::FunctionGenericity::OwnerParameterizedMethod {
            owner_parameters,
            no_gc_type_params,
            gc_free_pointee_requirements,
        } = &function.genericity
        else {
            return false;
        };
        let Some(method) = function.method else {
            return false;
        };
        let hir::FunctionKind::User(body) = &function.kind else {
            return false;
        };
        let [receiver] = function.params.as_slice() else {
            return false;
        };
        if receiver.local.into_raw().into_u32() as usize >= body.locals.len() {
            return false;
        }
        let receiver_local = &body.locals[receiver.local];

        declaration.name == "Iterable"
            && declaration.owner.is_none()
            && declaration.access.declared == hir::DeclaredVisibility::Public
            && parameter.bounds == hir::TypeParamBounds::Unconstrained
            && declaration.gc_free_pointee_requirements.is_empty()
            && declaration.parents.is_empty()
            && declaration.private_methods.is_empty()
            && declaration.properties.is_empty()
            && self_application.template == iterable
            && matches!(self_application.arguments.as_slice(), [argument] if matches!(self.types[*argument], Type::Param(found) if found == parameter.id))
            && self_application.canonical_type == receiver.ty
            && matches!(self.types[self_application.canonical_type], Type::Interface(found) if found == declaration.self_application)
            && relation.owner == iterable
            && relation.role == hir::InterfaceMemberRole::Function
            && relation.implementation == hir::InterfaceMemberImplementation::AbstractSlot
            && relation.overrides.is_empty()
            && function.name.rsplit('.').next() == Some("iterator")
            && function.access.declared == hir::DeclaredVisibility::Public
            && !function.is_suspend
            && function.attributes == hir::FunctionAttributes::default()
            && function.modifiers
                == hir::CallableModifiers {
                    operator: Some(hir::OperatorKind::Iterator),
                    ..hir::CallableModifiers::default()
                }
            && owner_parameters.as_slice() == declaration.type_params.as_slice()
            && no_gc_type_params.is_empty()
            && gc_free_pointee_requirements.is_empty()
            && method.owner == self_application.canonical_type
            && method.modifier == hir::MethodModifier::Abstract
            && method.dispatch == hir::MethodDispatch::Interface(*member)
            && receiver.name == "this"
            && receiver_local.name == receiver.name
            && receiver_local.ty == receiver.ty
            && !receiver_local.mutable
            && body.locals.len() == 1
            && body.statements.is_empty()
            && result.template == core.iterator()
            && result.arguments.as_slice() == self_application.arguments.as_slice()
            && result.canonical_type == function.return_ty
            && matches!(self.types[result.canonical_type], Type::Interface(found) if found == result_id)
    }
}
