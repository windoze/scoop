//! One MSC over all applicable typed function-like origins in a name layer.

use crate::{Lowerer, constructor_resolution::NamedNominalProbe, overload::NamedCallableProbe};
use scoop_ast as ast;
use scoop_hir as hir;

pub(crate) enum NamedFunctionLikeProbe {
    Callable(Box<NamedCallableProbe>),
    ImportedDependency(Box<crate::expr::ImportedDependencyCallProbe>),
    ImportedDerivedEquality(Box<crate::expr::ImportedDerivedEqualityProbe>),
    ImportedDependencyProperty(Box<crate::expr::ImportedDependencyExtensionPropertyProbe>),
    Nominal(Box<NamedNominalProbe>),
    IntrinsicStruct(NamedIntrinsicStructProbe),
}

pub(crate) struct NamedIntrinsicStructProbe {
    pub(crate) origin: NamedIntrinsicStructOrigin,
    pub(crate) fixed_alias: bool,
    pub(crate) span: ast::Span,
}

#[derive(Clone, Copy)]
pub(crate) enum NamedIntrinsicStructOrigin {
    Current(hir::StructId),
    Imported(hir::SourceNominalId),
}

impl NamedIntrinsicStructOrigin {
    pub(crate) fn type_parameters(
        self,
        state: &mut Lowerer,
        span: ast::Span,
    ) -> Result<Vec<hir::TypeParamDecl>, String> {
        match self {
            Self::Current(structure) => Ok(state.structs[structure].type_params.clone()),
            Self::Imported(owner) => {
                let declaration = state
                    .dependencies
                    .as_ref()
                    .and_then(|dependencies| dependencies.nominal_declaration(owner))
                    .cloned()
                    .expect("a resolved intrinsic type retains its actual declaration");
                let binders = declaration
                    .interface
                    .type_parameters()
                    .binders()
                    .iter()
                    .collect::<Vec<_>>();
                let keys = (0..binders.len())
                    .map(|index| scoop_identity::SignatureTypeKey::Binder {
                        depth: 0,
                        index: index as u32,
                    })
                    .collect::<Vec<_>>();
                state
                    .prepare_imported_type_parameters(&binders, &keys, span)
                    .map(|(parameters, _)| parameters)
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DeclarationDiagnosticOrder {
    Source(usize, u32, u32),
    ImportedIntrinsic(hir::SourceNominalId),
    Generated(scoop_identity::PersistentGeneratedCallableId),
}

impl NamedFunctionLikeProbe {
    fn forwarding(&self, state: &mut Lowerer) -> super::specificity::OwnedDeclarationForwarding {
        match self {
            Self::Callable(probe) => probe.forwarding().to_owned(),
            Self::ImportedDependency(probe) => probe.forwarding(state),
            Self::ImportedDerivedEquality(probe) => {
                super::specificity::DeclarationForwardingView::parameter_groups(
                    &[],
                    &[],
                    &[probe.owner],
                )
                .to_owned()
            }
            Self::ImportedDependencyProperty(probe) => probe.forwarding(state),
            Self::Nominal(probe) => probe.forwarding().to_owned(),
            Self::IntrinsicStruct(probe) => {
                let owners = if probe.fixed_alias {
                    Vec::new()
                } else {
                    probe
                        .origin
                        .type_parameters(state, probe.span)
                        .expect("an applicable intrinsic has resolved declaration parameters")
                };
                let ulong = state.integer_type(hir::IntegerKind::UNSIGNED_64);
                super::specificity::DeclarationForwardingView::nominal_parameters(&owners, &[ulong])
                    .to_owned()
            }
        }
    }
    fn parameterized(&self) -> bool {
        match self {
            Self::Callable(probe) => probe.parameterized(),
            Self::ImportedDependency(probe) => probe.parameterized(),
            Self::ImportedDerivedEquality(_) => false,
            Self::ImportedDependencyProperty(probe) => probe.parameterized(),
            Self::Nominal(probe) => probe.parameterized(),
            Self::IntrinsicStruct(probe) => !probe.fixed_alias,
        }
    }
    fn defaults(&self) -> usize {
        match self {
            Self::Callable(probe) => probe.defaults(),
            Self::ImportedDependency(probe) => probe.defaults(),
            Self::ImportedDerivedEquality(_) => 0,
            Self::ImportedDependencyProperty(_) => 0,
            Self::Nominal(probe) => probe.defaults(),
            Self::IntrinsicStruct(_) => 0,
        }
    }
    fn vararg(&self) -> bool {
        match self {
            Self::Callable(probe) => probe.vararg(),
            Self::ImportedDependency(probe) => probe.vararg(),
            Self::ImportedDerivedEquality(_) => false,
            Self::ImportedDependencyProperty(_) => false,
            Self::Nominal(probe) => probe.vararg(),
            Self::IntrinsicStruct(_) => false,
        }
    }
    fn source_argument_numeric(
        &self,
        index: usize,
    ) -> Option<super::specificity::NumericLiteralKind> {
        match self {
            Self::Callable(probe) => probe.source_argument_numeric(index),
            Self::ImportedDependency(probe) => probe.source_argument_numeric(index),
            Self::ImportedDerivedEquality(_) => None,
            Self::ImportedDependencyProperty(_) => None,
            Self::Nominal(probe) => probe.source_argument_numeric(index),
            Self::IntrinsicStruct(_) => Some(super::specificity::NumericLiteralKind::Integer(
                hir::IntegerKind::UNSIGNED_64,
            )),
        }
    }
    fn signature(&self, state: &Lowerer, name: &str) -> String {
        match self {
            Self::Callable(probe) => probe.signature(state, name),
            Self::ImportedDependency(probe) => probe.signature(name),
            Self::ImportedDerivedEquality(probe) => {
                let owner = state.type_name(probe.owner);
                format!("derived fun {owner}.{name}(other: {owner}): Boolean")
            }
            Self::ImportedDependencyProperty(probe) => probe.signature(name),
            Self::Nominal(probe) => probe.signature(state),
            Self::IntrinsicStruct(probe) => {
                let parameters = if probe.fixed_alias { "" } else { "<T>" };
                format!("{name}{parameters}(raw: ULong)")
            }
        }
    }

    fn diagnostic_order(&self, state: &Lowerer) -> DeclarationDiagnosticOrder {
        let (file, span) = match self {
            Self::Callable(probe) => probe.declaration_location(state),
            Self::ImportedDependency(probe) => probe.declaration_location(),
            Self::ImportedDerivedEquality(probe) => {
                return DeclarationDiagnosticOrder::Generated(probe.target.callable());
            }
            Self::ImportedDependencyProperty(probe) => probe.declaration_location(),
            Self::Nominal(probe) => return probe.diagnostic_order(state),
            Self::IntrinsicStruct(probe) => match probe.origin {
                NamedIntrinsicStructOrigin::Current(structure) => (
                    state.struct_files[&structure],
                    state.structs[structure].span,
                ),
                NamedIntrinsicStructOrigin::Imported(owner) => {
                    return DeclarationDiagnosticOrder::ImportedIntrinsic(owner);
                }
            },
        };
        DeclarationDiagnosticOrder::Source(file, span.start, span.end)
    }
}

impl Lowerer {
    pub(crate) fn select_named_function_like(
        &mut self,
        name: &str,
        layer: &str,
        probes: &[NamedFunctionLikeProbe],
        arguments: &[ast::CallArgument],
        span: ast::Span,
    ) -> Option<usize> {
        self.select_named_function_like_with_literals(name, layer, probes, Some(arguments), span)
    }

    pub(crate) fn select_lowered_named_function_like(
        &mut self,
        name: &str,
        layer: &str,
        probes: &[NamedFunctionLikeProbe],
        span: ast::Span,
    ) -> Option<usize> {
        self.select_named_function_like_with_literals(name, layer, probes, None, span)
    }

    fn select_named_function_like_with_literals(
        &mut self,
        name: &str,
        layer: &str,
        probes: &[NamedFunctionLikeProbe],
        arguments: Option<&[ast::CallArgument]>,
        span: ast::Span,
    ) -> Option<usize> {
        if probes.len() == 1 {
            return Some(0);
        }
        // Imported signatures belong to candidate-local arenas. Resolve their
        // declaration types together in comparison scratch, without committing
        // either candidate or comparing this call's inferred substitutions.
        let mut comparison = self.clone();
        let declarations = probes
            .iter()
            .map(|probe| probe.forwarding(&mut comparison))
            .collect::<Vec<_>>();
        let candidates = probes
            .iter()
            .zip(&declarations)
            .map(
                |(probe, declaration)| super::specificity::ApplicableDeclaration {
                    declaration: declaration.as_view(),
                    parameterized: probe.parameterized(),
                    defaults: probe.defaults(),
                    vararg: probe.vararg(),
                },
            )
            .collect::<Vec<_>>();
        let mut pool = comparison.most_specific_declarations(&candidates);
        if let Some(arguments) = arguments {
            pool = super::specificity::prefer_literal_defaults(
                &pool,
                arguments,
                |candidate, index| probes[candidate].source_argument_numeric(index),
            );
        }
        if let [winner] = pool.as_slice() {
            return Some(*winner);
        }
        let mut signatures = pool
            .iter()
            .map(|&candidate| {
                (
                    (probes[candidate].diagnostic_order(self), candidate),
                    probes[candidate].signature(self, name),
                )
            })
            .collect::<Vec<_>>();
        // This order is diagnostic-only; MSC above never observes it.
        signatures.sort_by_key(|(order, _)| *order);
        self.error(
            span,
            format!(
                "call to `{name}` is ambiguous in {layer} layer:\n{}",
                signatures
                    .iter()
                    .map(|(_, signature)| format!("  - {signature} — applicable; tied by MSC"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        );
        None
    }
}
