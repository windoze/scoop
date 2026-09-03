use super::*;

impl Lowerer {
    /// MSC selection (step 2) over two or more applicable candidates.
    /// Returns the winning prepared-candidate index, or records the ambiguity
    /// diagnostic and returns `None`.
    pub(super) fn most_specific(
        &mut self,
        name: &str,
        prepared: &[Candidate],
        applicable: &[usize],
        span: Span,
    ) -> Option<usize> {
        let mut forwards = vec![vec![false; applicable.len()]; applicable.len()];
        for a in 0..applicable.len() {
            forwards[a][a] = true;
            for b in 0..applicable.len() {
                if a == b {
                    continue;
                }
                let source = &prepared[applicable[a]];
                let target = &prepared[applicable[b]];
                forwards[a][b] = self.callable_forwards(
                    crate::call_resolution::specificity::ForwardingDeclaration {
                        view: &source.view,
                        parameter_types: &source.params,
                    },
                    crate::call_resolution::specificity::ForwardingDeclaration {
                        view: &target.view,
                        parameter_types: &target.params,
                    },
                );
            }
        }

        // Remove a candidate only when another candidate forwards to it and
        // the reverse direction fails. Mutually forwarding declarations stay
        // tied until the non-parameterized preference below.
        let pool = (0..applicable.len())
            .filter(|&candidate| {
                !(0..applicable.len()).any(|other| {
                    other != candidate && forwards[other][candidate] && !forwards[candidate][other]
                })
            })
            .collect::<Vec<_>>();
        let non_generic: Vec<usize> = pool
            .iter()
            .copied()
            .filter(|&a| {
                let candidate = &prepared[applicable[a]];
                candidate.own_type_param_count == 0
            })
            .collect();
        let pool = if non_generic.is_empty() {
            pool
        } else {
            non_generic
        };
        let mutually_forwarding = pool.iter().all(|&source| {
            pool.iter()
                .all(|&target| forwards[source][target] && forwards[target][source])
        });
        let pool = if mutually_forwarding {
            let minimum_defaults = pool
                .iter()
                .map(|&candidate| {
                    prepared[applicable[candidate]]
                        .argument_map
                        .as_ref()
                        .expect("an applicable candidate has an argument map")
                        .explicit_default_count()
                })
                .min()
                .expect("MSC receives at least two applicable candidates");
            pool.into_iter()
                .filter(|&candidate| {
                    prepared[applicable[candidate]]
                        .argument_map
                        .as_ref()
                        .expect("an applicable candidate has an argument map")
                        .explicit_default_count()
                        == minimum_defaults
                })
                .collect::<Vec<_>>()
        } else {
            pool
        };
        let pool = if mutually_forwarding && pool.len() > 1 {
            let non_vararg = pool
                .iter()
                .copied()
                .filter(|&candidate| {
                    !prepared[applicable[candidate]]
                        .view
                        .value_parameters
                        .iter()
                        .any(|parameter| {
                            matches!(
                                parameter.calling,
                                crate::defaults::SourceParameterCalling::Vararg { .. }
                            )
                        })
                })
                .collect::<Vec<_>>();
            if non_vararg.is_empty() {
                pool
            } else {
                non_vararg
            }
        } else {
            pool
        };
        if pool.len() == 1 {
            Some(applicable[pool[0]])
        } else {
            let tied = pool
                .iter()
                .map(|&candidate| applicable[candidate])
                .collect::<Vec<_>>();
            self.ambiguity_diagnostic(name, prepared, &tied, span);
            None
        }
    }
}
