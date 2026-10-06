//! Missing core encode requirements become ordinary source-owned members.
//! Pending work is consumed before Export HIR construction.

use super::interfaces::InterfaceMemberInstance;
use super::*;
use ast::Span;
use hir::FunctionKind;

mod containers;
mod declarations;
mod fields;
mod syntax;
mod variants;

impl Lowerer {
    pub(crate) fn lower_derived_encoding_bodies(&mut self) {
        for (function, owner, encodable) in std::mem::take(&mut self.derived_encoding_methods) {
            self.current_file = self.function_files[&function];
            let parameters = self.signatures[&function].type_params.clone();
            let previous_parameters = std::mem::replace(&mut self.type_params_in_scope, parameters);
            let previous_owner = self.current_owner.replace(owner);
            let before = self.diagnostics.len();
            let span = self.functions[function].span;
            let block = if let Some(block) = self.container_encoding_body(owner, span) {
                Some(block)
            } else {
                match owner {
                    Owner::Struct(structure) => self.encode_struct(structure, encodable, span),
                    Owner::Enum(enumeration) => self.encode_enum(enumeration, encodable, span),
                    Owner::Class(class) => self.encode_class(class, encodable, span),
                    Owner::Object(_) => {
                        self.error(span, "automatic encode requires a struct, enum, or final class without a class base; provide an explicit encode implementation for an object".into());
                        None
                    }
                    Owner::Interface(_) => {
                        unreachable!("an interface does not request a nominal method body")
                    }
                }
            };
            self.type_params_in_scope = previous_parameters;
            self.current_owner = previous_owner;
            if self.diagnostics.len() != before {
                continue;
            }
            if let Some(block) = block {
                let body =
                    self.lower_synthesized_body(function, |lowerer| lowerer.lower_block(&block));
                self.functions[function].kind = FunctionKind::User(body);
            }
        }
    }
}
