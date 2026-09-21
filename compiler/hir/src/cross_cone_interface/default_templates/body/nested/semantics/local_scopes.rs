use super::*;

impl<A: DefaultBodyNestedAuthority<E>, E> Validator<'_, A, E> {
    pub(super) fn enter_local_scope(
        &mut self,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.charge_work()?;
        self.meter
            .try_reserve_collection_slots(&mut self.local_scopes, 1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        self.local_scopes.push(HashSet::new());
        Ok(())
    }

    pub(super) fn leave_local_scope(
        &mut self,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.charge_work()?;
        self.local_scopes
            .pop()
            .expect("every scope exit follows a scope entry in the body walk");
        Ok(())
    }

    pub(super) fn record_local_declaration(
        &mut self,
        declaration: CallableTemplateOrigin,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.charge_work()?;
        let scope = self
            .local_scopes
            .last_mut()
            .expect("body declarations belong to an active lexical scope");
        if scope.contains(&declaration) {
            return Err(
                DefaultNestedCallableAbiValidationError::DuplicateLocalFunction { declaration },
            );
        }
        self.meter
            .try_reserve_set_slots(scope, 1, self.path)
            .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
        scope.insert(declaration);
        Ok(())
    }

    pub(super) fn record_local_use(
        &mut self,
        declaration: CallableTemplateOrigin,
        site: DefaultNestedCallableLocalUseV1,
    ) -> Result<(), DefaultNestedCallableAbiValidationError<E>> {
        self.charge_work()?;
        for scope in self.local_scopes.iter().rev() {
            self.meter
                .charge_work(1, self.path)
                .map_err(DefaultNestedCallableAbiValidationError::Resource)?;
            if scope.contains(&declaration) {
                return Ok(());
            }
        }
        Err(DefaultNestedCallableAbiValidationError::MissingLocalFunction { declaration, site })
    }
}
