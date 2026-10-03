use super::*;

use crate::call_resolution::specificity::ApplicableDeclaration;

impl Lowerer {
    /// Return the ordinary MSC pool before literal default-kind preference.
    pub(super) fn most_specific_candidates(
        &self,
        prepared: &[Candidate],
        applicable: &[usize],
    ) -> Vec<usize> {
        let declarations = applicable
            .iter()
            .map(|&index| {
                let candidate = &prepared[index];
                ApplicableDeclaration {
                    declaration: candidate.view.forwarding(&candidate.params),
                    parameterized: candidate.own_type_param_count != 0,
                    defaults: candidate
                        .argument_map
                        .as_ref()
                        .expect("an applicable candidate has an argument map")
                        .explicit_default_count(),
                    vararg: candidate
                        .view
                        .signature
                        .value_parameters
                        .iter()
                        .any(|parameter| parameter.is_vararg()),
                }
            })
            .collect::<Vec<_>>();
        self.most_specific_declarations(&declarations)
            .into_iter()
            .map(|index| applicable[index])
            .collect()
    }
}
