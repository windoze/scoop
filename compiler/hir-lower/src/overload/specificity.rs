use super::*;

impl Lowerer {
    /// MSC selection (step 2) over two or more applicable candidates.
    /// Returns the winning `(prepared index, type arguments)` pair, or
    /// records the ambiguity diagnostic and returns `None`.
    pub(super) fn most_specific(
        &mut self,
        name: &str,
        prepared: &[Candidate],
        applicable: &[(usize, Vec<TypeId>)],
        span: Span,
    ) -> Option<(usize, Vec<TypeId>)> {
        let mut forwards = vec![vec![false; applicable.len()]; applicable.len()];
        for a in 0..applicable.len() {
            forwards[a][a] = true;
            for b in 0..applicable.len() {
                if a == b {
                    continue;
                }
                let source = &prepared[applicable[a].0];
                let target = &prepared[applicable[b].0];
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
                let candidate = &prepared[applicable[a].0];
                candidate.own_type_param_count == 0
            })
            .collect();
        let pool = if non_generic.is_empty() {
            pool
        } else {
            non_generic
        };
        if pool.len() == 1 {
            Some(applicable[pool[0]].clone())
        } else {
            self.error(span, format!("call to `{name}` is ambiguous"));
            None
        }
    }
}
