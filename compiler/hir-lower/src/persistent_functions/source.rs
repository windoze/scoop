//! Source callable identities shared by fixed queries and final HIR assembly.

use super::*;

mod conditions;

pub(super) struct SourceFunctionContext {
    pub(super) source: scoop_identity::SourceIdentity,
    pub(super) package: PackagePath,
    pub(super) owners: Vec<DefinitionOwnerAtom>,
    pub(super) scope: DeclarationScope,
    pub(super) binders: Vec<SignatureBinder>,
}

pub(super) struct SourceIdentityBuilder<'a, 'mapper> {
    pub(super) lowerer: &'a Lowerer,
    pub(super) nominals: &'a hir::HirNominalIdentities,
    pub(super) type_mapper: &'mapper SignatureTypeMapper<'a>,
}

impl SourceIdentityBuilder<'_, '_> {
    pub(super) fn direct_identity(
        &self,
        function: hir::FunctionId,
        declaration: &SourceFunctionDeclaration,
    ) -> Result<hir::HirSourceFunctionIdentity, PersistentFunctionIdentityError> {
        let signature = self
            .lowerer
            .signatures
            .get(&function)
            .ok_or_else(|| self.failure(function, Detail::MissingSignature))?;
        let file = self.lowerer.function_files[&function];
        let source = self.source(function, file)?;
        let package = self.package(function, file)?;
        let owners = self.direct_source_owners(function, &source, &package)?;
        if signature.owner_type_param_count > signature.type_params.len() {
            return Err(self.failure(function, Detail::InvalidBinderInheritance));
        }
        let scope = if !self.lowerer.function_owner.contains_key(&function)
            && self.lowerer.functions[function].access.declared == hir::DeclaredVisibility::Private
        {
            DeclarationScope::SourceScoped(source.clone())
        } else {
            DeclarationScope::ConeWide
        };
        let owner = self.binder_group(
            function,
            &signature.type_params[..signature.owner_type_param_count],
        )?;
        let binders = self.append_binder_group(
            function,
            owner,
            &signature.type_params[signature.owner_type_param_count..],
        )?;
        self.identity(
            function,
            declaration,
            SourceFunctionContext {
                source,
                package,
                owners,
                scope,
                binders,
            },
        )
    }

    pub(super) fn identity(
        &self,
        function: hir::FunctionId,
        source_declaration: &SourceFunctionDeclaration,
        context: SourceFunctionContext,
    ) -> Result<hir::HirSourceFunctionIdentity, PersistentFunctionIdentityError> {
        let signature = self
            .lowerer
            .signatures
            .get(&function)
            .ok_or_else(|| self.failure(function, Detail::MissingSignature))?;
        let own_count = signature
            .type_params
            .len()
            .checked_sub(signature.owner_type_param_count)
            .ok_or_else(|| self.failure(function, Detail::InvalidBinderInheritance))?;
        let own_count_u32 = u32::try_from(own_count)
            .map_err(|_| self.failure(function, Detail::TooManyTypeParameters))?;
        let SourceFunctionContext {
            source,
            package,
            owners,
            scope,
            binders,
        } = context;
        let site = SourceDeclarationSite::new(
            source.cone(),
            package,
            DefinitionOwnerChain::from_outer_to_inner(owners),
            scope,
        )
        .map_err(|error| self.failure(function, Detail::InvalidSite(error)))?;
        let name = CanonicalIdentifier::new(&source_declaration.name)
            .map_err(|error| self.failure(function, Detail::InvalidName(error)))?;
        let receiver = self
            .lowerer
            .extension_receivers
            .get(&function)
            .copied()
            .map(|ty| self.type_mapper.map(ty, &binders))
            .transpose()
            .map_err(|error| self.failure(function, Detail::InvalidSignatureType(error)))?;
        let parameters = signature
            .params
            .iter()
            .map(|parameter| self.type_mapper.map(parameter.ty, &binders))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| self.failure(function, Detail::InvalidSignatureType(error)))?;
        let conditions = self.function_conditions(function, &binders)?;
        hir::HirSourceFunctionIdentity::from_declaration(
            SourceDeclarationKey::constrained_function(
                site,
                name,
                own_count_u32,
                receiver,
                parameters,
                conditions,
            ),
        )
        .map_err(|error| self.failure(function, Detail::InvalidIdentity(error)))
    }

    pub(super) fn binder_group(
        &self,
        function: hir::FunctionId,
        parameters: &[hir::TypeParamDecl],
    ) -> Result<Vec<SignatureBinder>, PersistentFunctionIdentityError> {
        parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| {
                Ok(SignatureBinder {
                    parameter: parameter.id,
                    depth: 0,
                    index: u32::try_from(index)
                        .map_err(|_| self.failure(function, Detail::TooManyTypeParameters))?,
                })
            })
            .collect()
    }

    pub(super) fn append_binder_group(
        &self,
        function: hir::FunctionId,
        mut outer: Vec<SignatureBinder>,
        inner: &[hir::TypeParamDecl],
    ) -> Result<Vec<SignatureBinder>, PersistentFunctionIdentityError> {
        if inner.is_empty() {
            return Ok(outer);
        }
        for binder in &mut outer {
            binder.depth = binder
                .depth
                .checked_add(1)
                .ok_or_else(|| self.failure(function, Detail::BinderDepthOverflow))?;
        }
        outer.extend(self.binder_group(function, inner)?);
        Ok(outer)
    }

    pub(super) fn direct_source_owners(
        &self,
        function: hir::FunctionId,
        source: &scoop_identity::SourceIdentity,
        package: &PackagePath,
    ) -> Result<Vec<DefinitionOwnerAtom>, PersistentFunctionIdentityError> {
        let Some(owner) = self.lowerer.function_owner.get(&function).copied() else {
            return Ok(Vec::new());
        };
        let identity = match owner {
            Owner::Class(owner) => &self.nominals[owner],
            Owner::Struct(owner) => &self.nominals[owner],
            Owner::Enum(owner) => &self.nominals[owner],
            Owner::Interface(owner) => &self.nominals[owner],
            Owner::Object(owner) => &self.nominals[owner],
        };
        let identity = identity
            .source()
            .ok_or_else(|| self.failure(function, Detail::GeneratedDeclarationOwner))?;
        if identity.declaration().origin() != source.cone()
            || identity.declaration().package() != package
        {
            return Err(self.failure(function, Detail::OwnerSiteMismatch));
        }
        let mut owners = identity.declaration().owners().owners().to_vec();
        owners.push(identity.definition_owner());
        Ok(owners)
    }

    pub(super) fn source(
        &self,
        function: hir::FunctionId,
        file: usize,
    ) -> Result<scoop_identity::SourceIdentity, PersistentFunctionIdentityError> {
        self.lowerer
            .intrinsic_sources
            .get(file)
            .map(|source| source.identity.clone())
            .ok_or_else(|| self.failure(function, Detail::MissingSourceFile))
    }

    pub(super) fn package(
        &self,
        function: hir::FunctionId,
        file: usize,
    ) -> Result<PackagePath, PersistentFunctionIdentityError> {
        match self.lowerer.top_level_namespaces.source_namespace(file) {
            TopLevelLookupLayer::CorePrelude => Ok(PackagePath::root()),
            TopLevelLookupLayer::CurrentPackage(package) => self
                .lowerer
                .top_level_namespaces
                .package_segments(package)
                .into_iter()
                .map(CanonicalIdentifier::new)
                .collect::<Result<Vec<_>, _>>()
                .map(PackagePath::from_segments)
                .map_err(|error| self.failure(function, Detail::InvalidName(error))),
        }
    }

    fn failure(
        &self,
        function: hir::FunctionId,
        detail: Detail,
    ) -> PersistentFunctionIdentityError {
        PersistentFunctionIdentityError {
            file: self.lowerer.function_files[&function],
            span: self.lowerer.functions[function].span,
            detail,
        }
    }
}
