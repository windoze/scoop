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
        // Dominance compares the parameter types with each candidate's
        // own inferred type arguments applied.
        let inst_params: Vec<Vec<TypeId>> = applicable
            .iter()
            .map(|(index, type_args)| {
                prepared[*index]
                    .params
                    .iter()
                    .map(|&param| self.substitute_call_level(param, type_args))
                    .collect()
            })
            .collect();
        let mut dominators = Vec::new();
        for a in 0..applicable.len() {
            let dominates_all = (0..applicable.len()).all(|b| {
                b == a
                    || inst_params[a]
                        .iter()
                        .zip(&inst_params[b])
                        .all(|(&x, &y)| self.is_subtype(x, y))
            });
            if dominates_all {
                dominators.push(a);
            }
        }
        if dominators.len() == 1 {
            return Some(applicable[dominators[0]].clone());
        }
        // A tie (mutual or no dominance): concrete candidates win over
        // parameterized ones; anything still tied is ambiguous.
        let pool = if dominators.is_empty() {
            (0..applicable.len()).collect::<Vec<_>>()
        } else {
            dominators
        };
        let non_generic: Vec<usize> = pool
            .iter()
            .copied()
            .filter(|&a| {
                let candidate = &prepared[applicable[a].0];
                candidate.own_type_param_count == 0 && !candidate.parameterized
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
