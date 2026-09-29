use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn materialize_imported_method_callee(
        &mut self,
        source: &hir::DefaultMethodCalleeV1,
        origin: hir::ExpressionOrigin,
        context: &ImportedDefaultContext<'_>,
    ) -> Result<hir::ImportedMethodCallee, ImportedDefaultMaterializationError> {
        match source {
            hir::DefaultMethodCalleeV1::Callable(callee) => self
                .materialize_imported_callable_target(callee, MemberCallKind::Ordinary, context)
                .map(hir::ImportedMethodCallee::Callable),
            hir::DefaultMethodCalleeV1::Bound(bound) => match bound.source() {
                hir::DefaultBoundCallableSourceV1::Class { callable, .. } => self
                    .materialize_imported_callable_target(
                        callable,
                        MemberCallKind::Ordinary,
                        context,
                    )
                    .map(hir::ImportedMethodCallee::Callable),
                hir::DefaultBoundCallableSourceV1::Interface {
                    bound: interface,
                    member,
                } => {
                    let interface = self.materialize_imported_default_type(interface, context)?;
                    let hir::Type::ImportedInterface(declaration) = &self.types[interface] else {
                        return Err(ImportedDefaultMaterializationError::Plan(
                            "an interface bound retains its declared interface application".into(),
                        ));
                    };
                    let slot = declaration
                        .methods
                        .iter()
                        .find(|method| method.declaration.declaration() == *member)
                        .map(|method| method.slot.id())
                        .ok_or_else(|| {
                            ImportedDefaultMaterializationError::Plan(
                                "an interface bound is missing its declared member slot".into(),
                            )
                        })?;
                    self.require_imported_bound_interface(interface)
                        .map_err(|error| {
                            ImportedDefaultMaterializationError::Plan(
                                error.diagnostic("bound conformance"),
                            )
                        })?;
                    let candidate = self
                        .dependencies
                        .as_ref()
                        .expect("a dependency bound retains its source catalog")
                        .callable_declaration(*member)
                        .map_err(|error| {
                            ImportedDefaultMaterializationError::DependencySelection(
                                error.to_string(),
                            )
                        })?;
                    let declared = if matches!(
                        candidate.interface().owner(),
                        hir::PublicDeclarationOwnerV1::Nominal(
                            hir::SourceNominalId::GenericTemplate(_)
                        )
                    ) {
                        match self
                            .resolve_imported_dispatch_callable(candidate, interface)
                            .map_err(|error| {
                                ImportedDefaultMaterializationError::Plan(
                                    error.diagnostic("bound member"),
                                )
                            })? {
                            hir::ImportedDispatchCallable::Template(application) => {
                                hir::ImportedCallableTarget::Application(application)
                            }
                            hir::ImportedDispatchCallable::External(callee) => {
                                hir::ImportedCallableTarget::Dependency(callee)
                            }
                        }
                    } else {
                        hir::ImportedCallableTarget::Dependency(
                            self.select_imported_callable_declaration_use_with_kind(
                                candidate,
                                MemberCallKind::Ordinary,
                            )
                            .map_err(|error| {
                                ImportedDefaultMaterializationError::DependencySelection(
                                    error.to_string(),
                                )
                            })?,
                        )
                    };
                    let receiver_type =
                        self.materialize_imported_default_type(bound.receiver_type(), context)?;
                    self.resolve_imported_member_receiver_type(receiver_type)
                        .map_err(|error| {
                            ImportedDefaultMaterializationError::Plan(
                                error.diagnostic("bound receiver"),
                            )
                        })?;
                    let signature = self.materialize_imported_default_type(
                        bound.instantiated_signature(),
                        context,
                    )?;
                    let hir::Type::Function(signature) = self.types[signature] else {
                        return Err(ImportedDefaultMaterializationError::Plan(
                            "a bound member retains its complete function signature".into(),
                        ));
                    };
                    Ok(hir::ImportedMethodCallee::InterfaceBound(Box::new(
                        hir::ImportedInterfaceBoundCallable {
                            receiver_type,
                            interface,
                            member: *member,
                            slot,
                            declared,
                            signature,
                        },
                    )))
                }
            },
            hir::DefaultMethodCalleeV1::DerivedEquality { owner_type } => {
                let owner = self.materialize_imported_default_type(owner_type, context)?;
                let candidate = self.derived_equality_candidate_at(owner, origin)
                    .map_err(ImportedDefaultMaterializationError::Plan)?
                    .ok_or_else(|| ImportedDefaultMaterializationError::Plan(
                        "a derived equality target requires an equality derivation for its owner".into(),
                    ))?;
                let (crate::derived::DerivedEqualityCandidate::Nominal { application, .. }
                | crate::derived::DerivedEqualityCandidate::TypeOwned { application, .. }) =
                    candidate;
                Ok(hir::ImportedMethodCallee::DerivedEquality(application))
            }
        }
    }
}
