use super::{ImportedCallableCandidate, ImportedDependencyCallReceiver, ImportedMemberReceiver};
use crate::Lowerer;
use crate::expr::named_calls::imported_dependency::inputs::ImportedCallReceiver;
use hir::ImportedCallableSource;
use scoop_ast as ast;
use scoop_hir as hir;

impl Lowerer {
    pub(super) fn imported_callable_receiver(
        &mut self,
        candidate: &ImportedCallableCandidate,
        name: &ast::Ident,
        span: ast::Span,
        receiver_source: ImportedDependencyCallReceiver,
        has_vararg: bool,
    ) -> Result<ImportedCallReceiver, Box<Lowerer>> {
        let interface = candidate.interface();
        if matches!(
            interface.declaration(),
            scoop_identity::CallableTemplateOrigin::Constructor(_)
        ) {
            return match receiver_source {
                ImportedDependencyCallReceiver::Implicit => Ok(ImportedCallReceiver::Absent),
                ImportedDependencyCallReceiver::Explicit(_) => {
                    self.error(
                        name.span,
                        format!("dependency constructor `{}` has no receiver", name.text),
                    );
                    Err(Box::new(self.clone()))
                }
            };
        }
        let receiver = match interface.owner() {
            hir::PublicDeclarationOwnerV1::TopLevel => match receiver_source {
                ImportedDependencyCallReceiver::Implicit => {
                    return Ok(ImportedCallReceiver::Absent);
                }
                ImportedDependencyCallReceiver::Explicit(_) => {
                    self.error(
                        name.span,
                        format!("dependency function `{}` is not an extension", name.text),
                    );
                    return Err(Box::new(self.clone()));
                }
            },
            hir::PublicDeclarationOwnerV1::Extension => {
                let Some(signature) = interface.receiver() else {
                    self.error(
                        name.span,
                        format!(
                            "invalid imported dependency extension `{}`: receiver type is missing",
                            name.text
                        ),
                    );
                    return Err(Box::new(self.clone()));
                };
                let receiver = match receiver_source {
                    ImportedDependencyCallReceiver::Implicit => {
                        let Some(receiver) = self.lower_current_this(name.span) else {
                            return Err(Box::new(self.clone()));
                        };
                        ImportedMemberReceiver::Value(receiver)
                    }
                    ImportedDependencyCallReceiver::Explicit(receiver) => receiver,
                };
                match self.imported_signature_type(signature) {
                    Ok(expected) => {
                        self.adapt_imported_receiver(receiver, expected, name, "extension")?
                    }
                    Err(_) => ImportedCallReceiver::Member {
                        static_type: receiver.ty(),
                        value: receiver,
                    },
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
                let hir::SourceNominalId::Concrete(owner) = owner else {
                    self.imported_dependency_capability_error(
                        candidate,
                        has_vararg,
                        "dependency member",
                        span,
                    );
                    return Err(Box::new(self.clone()));
                };
                let signature = scoop_identity::SignatureTypeKey::Nominal(owner);
                let Ok(expected) = self.imported_signature_type(&signature) else {
                    self.imported_dependency_capability_error(
                        candidate,
                        has_vararg,
                        "dependency member receiver",
                        span,
                    );
                    return Err(Box::new(self.clone()));
                };
                self.adapt_imported_receiver(receiver, expected, name, "member")?
            }
        };
        Ok(receiver)
    }

    fn adapt_imported_receiver(
        &mut self,
        receiver: ImportedMemberReceiver,
        expected: hir::TypeId,
        name: &ast::Ident,
        kind: &str,
    ) -> Result<ImportedCallReceiver, Box<Lowerer>> {
        if !self.is_subtype(receiver.ty(), expected) {
            self.error(
                name.span,
                format!(
                    "dependency {kind} `{}` expects receiver {}, found {}",
                    name.text,
                    self.type_name(expected),
                    self.type_name(receiver.ty())
                ),
            );
            return Err(Box::new(self.clone()));
        }
        let static_type = receiver.ty();
        let value = match receiver {
            ImportedMemberReceiver::Value(value) => {
                ImportedMemberReceiver::Value(self.adapt_to(value, expected))
            }
            ImportedMemberReceiver::LiteralSubject(_) => {
                ImportedMemberReceiver::LiteralSubject(expected)
            }
        };
        Ok(ImportedCallReceiver::Member { value, static_type })
    }
}
