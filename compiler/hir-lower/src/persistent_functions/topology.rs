use super::*;

impl FunctionIdentityBuilder<'_> {
    pub(super) fn accessor(
        &self,
        function: hir::FunctionId,
    ) -> Result<Option<hir::HirPropertyAccessorFunction>, PersistentFunctionIdentityError> {
        let mut found = None;
        for (getter, declaration) in self.lowerer.property_getters.iter() {
            if matches!(
                declaration.implementation,
                hir::PropertyAccessorImplementation::Body(actual)
                    | hir::PropertyAccessorImplementation::AbstractSlot(actual)
                    if actual == function
            ) {
                self.set_unique_origin(
                    function,
                    &mut found,
                    hir::HirPropertyAccessorFunction::Getter(getter),
                )?;
            }
        }
        for (setter, declaration) in self.lowerer.property_setters.iter() {
            if matches!(
                declaration.implementation,
                hir::PropertyAccessorImplementation::Body(actual)
                    | hir::PropertyAccessorImplementation::AbstractSlot(actual)
                    if actual == function
            ) {
                self.set_unique_origin(
                    function,
                    &mut found,
                    hir::HirPropertyAccessorFunction::Setter(setter),
                )?;
            }
        }
        Ok(found)
    }

    pub(super) fn lexical_site(
        &self,
        function: hir::FunctionId,
    ) -> Result<Option<LexicalSite>, PersistentFunctionIdentityError> {
        let mut found = None;
        for (_, declaration) in self.lowerer.lambdas.iter() {
            if declaration.function == function {
                self.set_unique_lexical_site(
                    function,
                    &mut found,
                    declaration.definition_root,
                    declaration.definition_path.clone(),
                    LexicalCallableRole::LambdaBody,
                )?;
            }
        }
        for (_, declaration) in self.lowerer.anonymous_functions.iter() {
            if declaration.function == function {
                self.set_unique_lexical_site(
                    function,
                    &mut found,
                    declaration.definition_root,
                    declaration.definition_path.clone(),
                    LexicalCallableRole::AnonymousFunctionBody,
                )?;
            }
        }
        Ok(found)
    }

    pub(super) fn initialization(
        &self,
        function: hir::FunctionId,
    ) -> Result<
        Option<(hir::InitializationUnitId, InitializationCallableRole)>,
        PersistentFunctionIdentityError,
    > {
        let mut found = None;
        for (unit, declaration) in self.lowerer.initialization_units.iter() {
            if declaration.initializer == function {
                self.set_unique_origin(
                    function,
                    &mut found,
                    (unit, InitializationCallableRole::Initializer),
                )?;
            }
            if declaration.ensure == function {
                self.set_unique_origin(
                    function,
                    &mut found,
                    (unit, InitializationCallableRole::Ensure),
                )?;
            }
        }
        Ok(found)
    }

    pub(super) fn immediate_parent(
        &self,
        function: hir::FunctionId,
        root: hir::LexicalDefinitionRoot,
        path: &StructuralDefinitionPath,
    ) -> Result<Option<hir::FunctionId>, PersistentFunctionIdentityError> {
        let mut candidate = None;
        for (_, declaration) in self.lowerer.local_functions.iter() {
            self.consider_parent(
                function,
                root,
                path,
                declaration.function,
                declaration.definition_root,
                &declaration.definition_path,
                &mut candidate,
            )?;
        }
        for (_, declaration) in self.lowerer.lambdas.iter() {
            self.consider_parent(
                function,
                root,
                path,
                declaration.function,
                declaration.definition_root,
                &declaration.definition_path,
                &mut candidate,
            )?;
        }
        for (_, declaration) in self.lowerer.anonymous_functions.iter() {
            self.consider_parent(
                function,
                root,
                path,
                declaration.function,
                declaration.definition_root,
                &declaration.definition_path,
                &mut candidate,
            )?;
        }
        Ok(candidate.map(|(_, parent)| parent))
    }

    #[allow(clippy::too_many_arguments)]
    fn consider_parent(
        &self,
        function: hir::FunctionId,
        root: hir::LexicalDefinitionRoot,
        path: &StructuralDefinitionPath,
        possible_parent: hir::FunctionId,
        possible_root: hir::LexicalDefinitionRoot,
        possible_path: &StructuralDefinitionPath,
        candidate: &mut Option<(usize, hir::FunctionId)>,
    ) -> Result<(), PersistentFunctionIdentityError> {
        let possible_segments = possible_path.segments();
        if possible_root != root
            || possible_segments.len() >= path.segments().len()
            || !path.segments().starts_with(possible_segments)
        {
            return Ok(());
        }
        if possible_parent == function {
            return Err(self.failure(function, Detail::CyclicLexicalParent));
        }
        match candidate {
            Some((length, parent)) if *length == possible_segments.len() => {
                if *parent != possible_parent {
                    return Err(self.failure(function, Detail::AmbiguousLexicalParent));
                }
            }
            Some((length, _)) if *length > possible_segments.len() => {}
            _ => *candidate = Some((possible_segments.len(), possible_parent)),
        }
        Ok(())
    }

    fn set_unique_origin<T: Copy + Eq>(
        &self,
        function: hir::FunctionId,
        found: &mut Option<T>,
        value: T,
    ) -> Result<(), PersistentFunctionIdentityError> {
        if found.is_some_and(|found| found != value) {
            Err(self.failure(function, Detail::InvalidFunctionOrigin))
        } else {
            *found = Some(value);
            Ok(())
        }
    }

    fn set_unique_lexical_site(
        &self,
        function: hir::FunctionId,
        found: &mut Option<LexicalSite>,
        root: hir::LexicalDefinitionRoot,
        path: StructuralDefinitionPath,
        role: LexicalCallableRole,
    ) -> Result<(), PersistentFunctionIdentityError> {
        if found
            .as_ref()
            .is_some_and(|found| found.root != root || found.path != path || found.role != role)
        {
            return Err(self.failure(function, Detail::InvalidFunctionOrigin));
        }
        *found = Some(LexicalSite { root, path, role });
        Ok(())
    }
}
