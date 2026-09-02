use super::*;

impl Lowerer {
    // --- member lookup helpers ---

    fn direct_base_class(&self, class: ClassId) -> Option<ClassId> {
        let (base, _) = self.classes[class].base_class.as_ref()?;
        let Type::Class(application) = self.types[*base] else {
            unreachable!("resolved class bases are class applications")
        };
        Some(self.class_applications[application].template)
    }

    /// Every interface implemented by class `c` or its base classes,
    /// deduplicated, own list first (cycle-safe).
    pub(crate) fn class_interfaces_all(&mut self, c: ClassId) -> Vec<TypeId> {
        self.class_interfaces_for_application(self.classes[c].self_application)
    }

    /// The methods of the base classes of `c`, nearest base first
    /// (cycle-safe).
    pub(super) fn base_chain_methods(&mut self, c: ClassId) -> Vec<crate::CallableCandidate> {
        let mut result = Vec::new();
        let mut current = self.classes[c].self_application;
        let mut seen = vec![current];
        loop {
            let application = self.class_applications[current].clone();
            let Some((base, _)) = self.classes[application.template].base_class.clone() else {
                break;
            };
            let base = self.instantiate_ty(base, &application.arguments);
            let Type::Class(base_application) = self.types[base] else {
                unreachable!("class bases are resolved class applications")
            };
            if seen.contains(&base_application) {
                break;
            }
            seen.push(base_application);
            let base = self.class_applications[base_application].clone();
            result.extend(
                self.classes[base.template]
                    .methods
                    .iter()
                    .copied()
                    .map(|function| {
                        crate::CallableCandidate::method(
                            function,
                            hir::MethodOwnerApplication::Class(base_application),
                        )
                    }),
            );
            current = base_application;
        }
        result
    }

    /// The total number of constructor properties in the base chain of
    /// `c` — the layout offset of `c`'s own properties (base fields
    /// prefix, cycle-safe).
    pub(crate) fn base_field_total(&self, c: ClassId) -> u32 {
        let mut total = 0;
        let mut seen = vec![c];
        let mut current = Some(c);
        while let Some(id) = current {
            current = match self.direct_base_class(id) {
                Some(base) if !seen.contains(&base) => {
                    seen.push(base);
                    total += self.classes[base].semantic_constructor().len() as u32;
                    Some(base)
                }
                _ => None,
            };
        }
        total
    }

    /// Find a constructor property by name on class `c` or its base
    /// chain. Returns the declaring class, the absolute layout index
    /// (base fields prefix + own fields, consecutive), the type and
    /// the mutability.
    pub(crate) fn find_class_field(
        &self,
        c: ClassId,
        name: &str,
    ) -> Option<(ClassId, u32, TypeId, bool)> {
        if let Some(index) = self.classes[c]
            .semantic_constructor()
            .iter()
            .position(|field| field.name == name)
        {
            let abs = self.base_field_total(c) + index as u32;
            let ty = self.classes[c].semantic_constructor()[index].ty;
            let mutable = self.classes[c].semantic_constructor()[index].mutable;
            return Some((c, abs, ty, mutable));
        }
        let base = self.direct_base_class(c)?;
        self.find_class_field(base, name)
    }

    /// Field lookup on a complete class application. The declaration/layout
    /// identity remains the declaring `ClassId`, while the returned field type
    /// is fully substituted through every generic base application.
    pub(crate) fn find_class_application_field(
        &mut self,
        application: hir::ClassApplicationId,
        name: &str,
    ) -> Option<(hir::ClassApplicationId, u32, TypeId, bool)> {
        let application_value = self.class_applications[application].clone();
        let class = application_value.template;
        if let Some(index) = self.classes[class]
            .semantic_constructor()
            .iter()
            .position(|field| field.name == name)
        {
            let abs = self.base_field_total(class) + index as u32;
            let field_ty = self.classes[class].semantic_constructor()[index].ty;
            let ty = self.instantiate_ty(field_ty, &application_value.arguments);
            let mutable = self.classes[class].semantic_constructor()[index].mutable;
            return Some((application, abs, ty, mutable));
        }
        let (base, _) = self.classes[class].base_class.clone()?;
        let base = self.instantiate_ty(base, &application_value.arguments);
        let Type::Class(base_application) = self.types[base] else {
            unreachable!("resolved class bases are class applications")
        };
        self.find_class_application_field(base_application, name)
    }
}
