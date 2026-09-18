//! One MSC over all applicable typed function-like origins in a name layer.

use crate::{Lowerer, constructor_resolution::NamedNominalProbe, overload::NamedCallableProbe};
use scoop_ast as ast;
use scoop_hir as hir;

pub(crate) enum NamedFunctionLikeProbe {
    Callable(Box<NamedCallableProbe>),
    ImportedDependency(Box<crate::expr::ImportedDependencyCallProbe>),
    ImportedDependencyProperty(Box<crate::expr::ImportedDependencyExtensionPropertyProbe>),
    Nominal(Box<NamedNominalProbe>),
    IntrinsicStruct(NamedIntrinsicStructProbe),
}

pub(crate) struct NamedIntrinsicStructProbe {
    pub(crate) structure: hir::StructId,
    pub(crate) owners: Vec<hir::TypeParamDecl>,
    pub(crate) parameter_types: Vec<hir::TypeId>,
    pub(crate) integer_arguments: Vec<Option<hir::IntegerKind>>,
}

impl NamedFunctionLikeProbe {
    fn forwarding(&self) -> super::specificity::DeclarationForwardingView<'_> {
        match self {
            Self::Callable(probe) => probe.forwarding(),
            Self::ImportedDependency(probe) => probe.forwarding(),
            Self::ImportedDependencyProperty(probe) => probe.forwarding(),
            Self::Nominal(probe) => probe.forwarding(),
            Self::IntrinsicStruct(probe) => {
                super::specificity::DeclarationForwardingView::nominal_parameters(
                    &probe.owners,
                    &probe.parameter_types,
                )
            }
        }
    }
    fn parameterized(&self) -> bool {
        match self {
            Self::Callable(probe) => probe.parameterized(),
            Self::ImportedDependency(probe) => probe.parameterized(),
            Self::ImportedDependencyProperty(probe) => probe.parameterized(),
            Self::Nominal(probe) => probe.parameterized(),
            Self::IntrinsicStruct(probe) => !probe.owners.is_empty(),
        }
    }
    fn defaults(&self) -> usize {
        match self {
            Self::Callable(probe) => probe.defaults(),
            Self::ImportedDependency(probe) => probe.defaults(),
            Self::ImportedDependencyProperty(_) => 0,
            Self::Nominal(probe) => probe.defaults(),
            Self::IntrinsicStruct(_) => 0,
        }
    }
    fn vararg(&self) -> bool {
        match self {
            Self::Callable(probe) => probe.vararg(),
            Self::ImportedDependency(probe) => probe.vararg(),
            Self::ImportedDependencyProperty(_) => false,
            Self::Nominal(probe) => probe.vararg(),
            Self::IntrinsicStruct(_) => false,
        }
    }
    fn source_argument_integer(&self, index: usize) -> Option<hir::IntegerKind> {
        match self {
            Self::Callable(probe) => probe.source_argument_integer(index),
            Self::ImportedDependency(probe) => probe.source_argument_integer(index),
            Self::ImportedDependencyProperty(_) => None,
            Self::Nominal(probe) => probe.source_argument_integer(index),
            Self::IntrinsicStruct(probe) => probe.integer_arguments[index],
        }
    }
    fn signature(&self, state: &Lowerer, name: &str) -> String {
        match self {
            Self::Callable(probe) => probe.signature(state, name),
            Self::ImportedDependency(probe) => probe.signature(state, name),
            Self::ImportedDependencyProperty(probe) => probe.signature(state, name),
            Self::Nominal(probe) => probe.signature(state),
            Self::IntrinsicStruct(probe) => {
                format!("{}<T>(raw: ULong)", state.structs[probe.structure].name)
            }
        }
    }

    fn declaration_location(&self, state: &Lowerer) -> (usize, ast::Span) {
        match self {
            Self::Callable(probe) => probe.declaration_location(state),
            Self::ImportedDependency(probe) => probe.declaration_location(),
            Self::ImportedDependencyProperty(probe) => probe.declaration_location(),
            Self::Nominal(probe) => probe.declaration_location(state),
            Self::IntrinsicStruct(probe) => (
                state.struct_files[&probe.structure],
                state.structs[probe.structure].span,
            ),
        }
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
        let mut forwards = vec![vec![false; probes.len()]; probes.len()];
        for (source, row) in forwards.iter_mut().enumerate() {
            for (target, value) in row.iter_mut().enumerate() {
                *value = source == target
                    || self.declaration_forwards(
                        probes[source].forwarding(),
                        probes[target].forwarding(),
                    );
            }
        }
        let mut pool = (0..probes.len())
            .filter(|&candidate| {
                !(0..probes.len()).any(|other| {
                    other != candidate && forwards[other][candidate] && !forwards[candidate][other]
                })
            })
            .collect::<Vec<_>>();
        if pool
            .iter()
            .any(|&candidate| !probes[candidate].parameterized())
        {
            pool.retain(|&candidate| !probes[candidate].parameterized());
        }
        let mutually_forwarding = pool.iter().all(|&source| {
            pool.iter()
                .all(|&target| forwards[source][target] && forwards[target][source])
        });
        if mutually_forwarding
            && let Some(defaults) = pool
                .iter()
                .map(|&candidate| probes[candidate].defaults())
                .min()
        {
            pool.retain(|&candidate| probes[candidate].defaults() == defaults);
            if pool.iter().any(|&candidate| !probes[candidate].vararg()) {
                pool.retain(|&candidate| !probes[candidate].vararg());
            }
        }
        let ordinary = pool.clone();
        pool.retain(|&candidate| {
            !ordinary.iter().any(|&other| {
                other != candidate
                    && literal_dominates(&probes[other], &probes[candidate], arguments)
            })
        });
        if let [winner] = pool.as_slice() {
            return Some(*winner);
        }
        let mut signatures = pool
            .iter()
            .map(|&candidate| {
                let (file, span) = probes[candidate].declaration_location(self);
                (
                    (file, span.start, span.end, candidate),
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

fn literal_dominates(
    preferred: &NamedFunctionLikeProbe,
    other: &NamedFunctionLikeProbe,
    arguments: &[ast::CallArgument],
) -> bool {
    let mut better = false;
    for (index, argument) in arguments.iter().enumerate() {
        let Some(default) = crate::expr::integer_literal_default_kind(&argument.expression) else {
            continue;
        };
        let (Some(preferred), Some(other)) = (
            preferred.source_argument_integer(index),
            other.source_argument_integer(index),
        ) else {
            continue;
        };
        if preferred == other {
            continue;
        }
        match (preferred == default, other == default) {
            (true, false) => better = true,
            (false, true) => return false,
            _ => {}
        }
    }
    better
}
