use super::*;

impl HirSignatureTypeMapper<'_> {
    pub(super) fn map_struct(
        &self,
        owner: crate::StructId,
        arguments: &[TypeId],
        binders: &ParameterEnvironment<'_>,
        visiting: &mut HashSet<TypeId>,
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        if local_index(owner) >= self.inputs.structs.len() {
            return Err(HirSignatureTypeMappingError::UnknownNominal(raw_index(
                owner,
            )));
        }
        self.map_nominal(
            &self.inputs.nominal_identities[owner],
            self.inputs.structs[owner].type_params.len(),
            arguments,
            binders,
            visiting,
        )
    }

    pub(super) fn map_class(
        &self,
        owner: crate::ClassId,
        arguments: &[TypeId],
        binders: &ParameterEnvironment<'_>,
        visiting: &mut HashSet<TypeId>,
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        if local_index(owner) >= self.inputs.classes.len() {
            return Err(HirSignatureTypeMappingError::UnknownNominal(raw_index(
                owner,
            )));
        }
        let mut objects = self
            .inputs
            .objects
            .iter()
            .filter_map(|(object, declaration)| {
                (declaration.backing_class == owner).then_some(object)
            });
        let object = objects.next();
        if objects.next().is_some() {
            return Err(HirSignatureTypeMappingError::DuplicateObjectBackingClass(
                raw_index(owner),
            ));
        }
        let (identity, arity) = match object {
            Some(object) => (
                self.object_identity(object)?,
                self.inputs.classes[owner].type_params.len(),
            ),
            None => (
                &self.inputs.nominal_identities[owner],
                self.inputs.classes[owner].type_params.len(),
            ),
        };
        self.map_nominal(identity, arity, arguments, binders, visiting)
    }

    fn object_identity(
        &self,
        object: ObjectId,
    ) -> Result<&HirNominalIdentity, HirSignatureTypeMappingError> {
        if local_index(object) >= self.inputs.objects.len() {
            return Err(HirSignatureTypeMappingError::UnknownNominal(raw_index(
                object,
            )));
        }
        Ok(&self.inputs.nominal_identities[object])
    }

    pub(super) fn map_nominal(
        &self,
        identity: &HirNominalIdentity,
        arity: usize,
        arguments: &[TypeId],
        binders: &ParameterEnvironment<'_>,
        visiting: &mut HashSet<TypeId>,
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        if arguments.len() != arity {
            return Err(HirSignatureTypeMappingError::NominalArity {
                expected: arity,
                actual: arguments.len(),
            });
        }
        let Some(source) = identity.source() else {
            return Err(HirSignatureTypeMappingError::GeneratedNominal);
        };
        match source {
            HirSourceNominalIdentity::Concrete(record) => {
                if arity == 0 {
                    Ok(SignatureTypeKey::Nominal(record.id()))
                } else {
                    Err(HirSignatureTypeMappingError::NominalIdentityKind)
                }
            }
            HirSourceNominalIdentity::Generic(record) => {
                if arity == 0 {
                    return Err(HirSignatureTypeMappingError::NominalIdentityKind);
                }
                let arguments = NonEmptyVec::new(self.map_all(arguments, binders, visiting)?)
                    .expect("a generic nominal with positive arity has arguments");
                Ok(SignatureTypeKey::NominalApplication {
                    origin: record.id(),
                    arguments,
                })
            }
        }
    }

    pub(super) fn map_all(
        &self,
        types: &[TypeId],
        binders: &ParameterEnvironment<'_>,
        visiting: &mut HashSet<TypeId>,
    ) -> Result<Vec<SignatureTypeKey>, HirSignatureTypeMappingError> {
        types
            .iter()
            .map(|ty| self.map_inner(*ty, binders, visiting))
            .collect()
    }
}
