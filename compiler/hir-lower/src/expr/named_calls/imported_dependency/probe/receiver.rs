use super::{ImportedCallableCandidate, ImportedDependencyCallReceiver};
use crate::Lowerer;
use hir::ImportedCallableSource;
use scoop_ast as ast;
use scoop_hir as hir;

impl Lowerer {
    pub(super) fn imported_callable_receiver(
        &mut self,
        candidate: &ImportedCallableCandidate,
        name: &ast::Ident,
        call: crate::expr::CallSite<'_>,
        receiver_source: ImportedDependencyCallReceiver,
        has_vararg: bool,
    ) -> Result<Option<hir::Expr>, Box<Lowerer>> {
        let interface = candidate.interface();
        let receiver = match interface.owner() {
            hir::PublicDeclarationOwnerV1::TopLevel => match receiver_source {
                ImportedDependencyCallReceiver::Implicit => None,
                ImportedDependencyCallReceiver::Explicit(_) => {
                    self.error(
                        name.span,
                        format!("dependency function `{}` is not an extension", name.text),
                    );
                    return Err(Box::new(self.clone()));
                }
            },
            hir::PublicDeclarationOwnerV1::Extension => {
                let Some(receiver_signature) = interface.receiver() else {
                    self.error(
                        name.span,
                        format!(
                            "invalid imported dependency extension `{}`: receiver type is missing",
                            name.text
                        ),
                    );
                    return Err(Box::new(self.clone()));
                };
                let receiver_type = self.imported_signature_type(receiver_signature).ok();
                let receiver = match receiver_source {
                    ImportedDependencyCallReceiver::Implicit => {
                        let Some(receiver) = self.lower_current_this(name.span) else {
                            return Err(Box::new(self.clone()));
                        };
                        receiver
                    }
                    ImportedDependencyCallReceiver::Explicit(receiver) => receiver,
                };
                if let Some(receiver_type) = receiver_type {
                    if !self.is_subtype(receiver.ty, receiver_type) {
                        self.error(
                            name.span,
                            format!(
                                "dependency extension `{}` expects receiver {}, found {}",
                                name.text,
                                self.type_name(receiver_type),
                                self.type_name(receiver.ty),
                            ),
                        );
                        return Err(Box::new(self.clone()));
                    }
                    Some(self.adapt_to(receiver, receiver_type))
                } else {
                    Some(receiver)
                }
            }
            hir::PublicDeclarationOwnerV1::Nominal(owner) => {
                let ImportedDependencyCallReceiver::Explicit(receiver) = receiver_source else {
                    self.error(
                        name.span,
                        format!("dependency member `{}` requires a receiver", name.text),
                    );
                    return Err(Box::new(self.clone()));
                };
                let scoop_hir::SourceNominalId::Concrete(owner) = owner else {
                    self.imported_dependency_capability_error(
                        candidate,
                        has_vararg,
                        "dependency member",
                        call.span,
                    );
                    return Err(Box::new(self.clone()));
                };
                let signature = scoop_identity::SignatureTypeKey::Nominal(owner);
                let Ok(receiver_type) = self.imported_signature_type(&signature) else {
                    self.imported_dependency_capability_error(
                        candidate,
                        has_vararg,
                        "dependency member receiver",
                        call.span,
                    );
                    return Err(Box::new(self.clone()));
                };
                if !self.is_subtype(receiver.ty, receiver_type) {
                    self.error(
                        name.span,
                        format!(
                            "dependency member `{}` expects receiver {}, found {}",
                            name.text,
                            self.type_name(receiver_type),
                            self.type_name(receiver.ty)
                        ),
                    );
                    return Err(Box::new(self.clone()));
                }
                Some(self.adapt_to(receiver, receiver_type))
            }
        };
        Ok(receiver)
    }
}
