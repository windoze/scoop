//! Conditional relations use the original nominal and its parameter binder.

use crate::{FunctionId, Lowerer, Owner, Type, TypeId};
use scoop_ast as ast;
use scoop_hir as hir;

mod declarations;
mod identity;
mod negative;
mod overlap;
mod properties;
mod queries;
mod recursion;

#[derive(Clone, Default)]
pub(crate) struct ConditionalInterfaceState {
    pending: Vec<PendingImplementation>,
    active_queries: Vec<(hir::ConditionalInterfaceId, Vec<TypeId>)>,
    relation_depth: usize,
    cycles: Vec<ConditionalProofCycle>,
}

#[derive(Clone)]
struct ConditionalProofCycle {
    interface: TypeId,
    diagnostic: ast::Diagnostic,
}

#[derive(Clone)]
struct PendingImplementation {
    owner: Owner,
    declaration: ast::InterfaceImplementationDecl,
    functions: Vec<FunctionId>,
    file: usize,
}

impl Lowerer {
    pub(crate) fn declare_interface_block<'a>(
        &mut self,
        owner: Owner,
        declaration: &'a ast::InterfaceImplementationDecl,
        pending_methods: &mut Vec<(FunctionId, &'a ast::FunctionDecl, usize, Owner)>,
        file: usize,
    ) {
        let first = pending_methods.len();
        for method in declaration.functions() {
            self.declare_method(method, owner, pending_methods, file);
        }
        self.conditional_interfaces
            .pending
            .push(PendingImplementation {
                owner,
                declaration: declaration.clone(),
                functions: pending_methods[first..]
                    .iter()
                    .map(|&(function, ..)| function)
                    .collect(),
                file,
            });
    }

    pub(crate) fn owner_conditional_interfaces(
        &self,
        owner: Owner,
    ) -> &[hir::ConditionalInterfaceDeclaration] {
        match owner {
            Owner::Class(id) => &self.classes[id].conditional_interfaces,
            Owner::Object(id) => {
                &self.classes[self.objects[id].backing_class].conditional_interfaces
            }
            Owner::Struct(id) => &self.structs[id].conditional_interfaces,
            Owner::Enum(id) => &self.enums[id].conditional_interfaces,
            Owner::Interface(_) => &[],
        }
    }

    pub(crate) fn method_type_parameter_environment(
        &self,
        function: FunctionId,
        owner: Owner,
    ) -> Vec<hir::TypeParamDecl> {
        match self.functions[function]
            .method
            .and_then(|method| method.availability)
        {
            None => self.owner_type_params(owner),
            Some(condition) => self
                .owner_conditional_interfaces(owner)
                .iter()
                .find(|declaration| declaration.id() == condition)
                .expect("a conditional method retains its declaring implementation")
                .parameters
                .clone(),
        }
    }
}
