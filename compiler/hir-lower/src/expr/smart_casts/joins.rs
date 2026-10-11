use super::*;

impl Lowerer {
    pub(crate) fn add_flow_type(
        &mut self,
        facts: &mut SmartCastFacts,
        binding: hir::BindingId,
        ty: TypeId,
    ) {
        if facts.escaped.contains(&binding) {
            return;
        }
        for binding in facts.related(binding) {
            self.add_smart_cast_type(facts.types.entry(binding).or_default(), ty);
        }
    }

    pub(crate) fn join_flow_types(&mut self, facts: &mut SmartCastFacts, added: TypeFacts) {
        for (binding, types) in added {
            for ty in types {
                self.add_flow_type(facts, binding, ty);
            }
        }
    }

    pub(crate) fn common_smart_casts(
        &mut self,
        lhs: SmartCastFacts,
        rhs: SmartCastFacts,
    ) -> SmartCastFacts {
        let types = self.common_flow_types(&lhs.types, &rhs.types);
        let mut result = SmartCastFacts {
            types,
            aliases: lhs.aliases.intersection(&rhs.aliases).copied().collect(),
            escaped: lhs.escaped.union(&rhs.escaped).copied().collect(),
            ..Default::default()
        };
        let bindings: std::collections::BTreeSet<_> = lhs
            .booleans
            .keys()
            .chain(rhs.booleans.keys())
            .copied()
            .collect();
        for binding in bindings {
            let yes = self.common_boolean_path(&lhs, &rhs, binding, true);
            let no = self.common_boolean_path(&lhs, &rhs, binding, false);
            result.booleans.insert(binding, BooleanFacts { yes, no });
        }
        for binding in result.escaped.clone() {
            result.forget(binding);
        }
        result
    }

    fn common_boolean_path(
        &mut self,
        lhs: &SmartCastFacts,
        rhs: &SmartCastFacts,
        binding: hir::BindingId,
        outcome: bool,
    ) -> Option<TypeFacts> {
        let left = self.boolean_path_types(lhs, binding, outcome);
        let right = self.boolean_path_types(rhs, binding, outcome);
        match (left, right) {
            (Some(left), Some(right)) => Some(self.common_flow_types(&left, &right)),
            (left, right) => left.or(right),
        }
    }

    fn boolean_path_types(
        &mut self,
        facts: &SmartCastFacts,
        binding: hir::BindingId,
        outcome: bool,
    ) -> Option<TypeFacts> {
        let mut path = facts.clone();
        if let Some(boolean) = facts.booleans.get(&binding) {
            let added = if outcome { &boolean.yes } else { &boolean.no };
            self.join_flow_types(&mut path, added.clone()?);
        }
        Some(path.types)
    }

    fn common_flow_types(&mut self, lhs: &TypeFacts, rhs: &TypeFacts) -> TypeFacts {
        let mut result = TypeFacts::new();
        for (&binding, left) in lhs {
            let Some(right) = rhs.get(&binding) else {
                continue;
            };
            let mut common = Vec::new();
            for &left in left {
                for &right in right {
                    let ty = self.least_upper_bound(&[left, right]);
                    if !self.types_equal(ty, self.any) {
                        self.add_smart_cast_type(&mut common, ty);
                    }
                }
            }
            if !common.is_empty() {
                result.insert(binding, common);
            }
        }
        result
    }

    fn add_smart_cast_type(&mut self, known: &mut Vec<TypeId>, ty: TypeId) {
        if known.iter().any(|&before| self.is_subtype(before, ty)) {
            return;
        }
        known.retain(|&before| !self.is_subtype(ty, before));
        known.push(ty);
        known.sort();
    }
}
