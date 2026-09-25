use super::*;
use std::collections::BTreeMap;

pub(super) struct Producer<'a> {
    source: BTreeMap<PersistentConstructorId, &'a hir::CallableDeclarationRecordV1>,
    input: &'a mir::SingleConeStrongMirInput,
    authority: mir::MirCallableBridgeAuthority<'a>,
    records: Vec<mir::ParamFreeMirCallableBindingV1>,
}
impl<'a> Producer<'a> {
    pub(super) fn new(
        public: &'a hir::CrossConeHirInterfaceSectionV1,
        input: &'a mir::SingleConeStrongMirInput,
        identities: &'a ValidatedIdentityGraph,
        types: &'a dyn mir::MirTypeBridgeTypeLookupV1,
    ) -> Result<Self, Error> {
        let source =
            hir::select_param_free_source_constructors(input.module().cone, public, identities)?;
        let mut records = Vec::new();

        scoop_wire::allocation::try_reserve(&mut records, source.len(), &WirePath::root())?;
        Ok(Self {
            source,
            input,
            authority: mir::MirCallableBridgeAuthority {
                identities,
                foundation: input.foundation(),
                types,
            },
            records,
        })
    }

    pub(super) fn source(
        &mut self,
        materialization: CallableMaterialization,
    ) -> Result<
        Option<(
            PersistentConstructorId,
            &'a hir::CallableDeclarationRecordV1,
        )>,
        Error,
    > {
        let CallableTemplateOwner::Constructor(declaration) = materialization.template() else {
            return Ok(None);
        };
        let Some(&source) = self.source.get(&declaration) else {
            return Ok(None);
        };
        if materialization.context() != CallableMaterializationContext::NoSubstitution {
            return Err(Error::OdrRequired(declaration));
        }
        Ok(Some((declaration, source)))
    }

    pub(super) fn record(
        &mut self,
        declaration: PersistentConstructorId,
        source: &hir::CallableDeclarationRecordV1,
        semantic: ExactCallableSignature,
        lowered: ExactCallableSignature,
        role: mir::MirCallableLoweringRoleV1,
    ) -> Result<(), Error> {
        let roots = self.input.materialization().callable_roots();
        let signatures = &self.input.module().meta.callable_signatures;

        let index = roots
            .binary_search_by_key(&CallableOwner::Constructor(declaration), |root| {
                root.implementation()
            })
            .map_err(|_| Error::MissingMaterialization(declaration))?;
        let root = roots[index];
        let actual = signatures
            .get(mir::CallableSignatureSubject::Strong(root.implementation()))
            .ok_or(Error::MissingSignature(declaration))?;
        if actual.signature() != &lowered {
            return Err(Error::SourceSignatureMismatch(declaration));
        }
        let source_gc = match source.effects().gc_effect() {
            scoop_identity::GcEffect::Managed => mir::GcEffect::Managed,
            scoop_identity::GcEffect::NoGc => mir::GcEffect::NoGc,
        };
        self.records
            .push(mir::ParamFreeMirCallableBindingV1::try_new(
                self.authority,
                mir::MirCallableOriginV1::Constructor(declaration),
                StrongCallableDefinitionOwner::Constructor(declaration),
                mir::MirBridgeCallableSignatureV1::new(semantic, source_gc),
                mir::MirBridgeCallableSignatureV1::new(
                    lowered,
                    self.input.module().functions[root.function()].gc_effect,
                ),
                role,
            )?);
        Ok(())
    }

    pub(super) fn finish(self) -> Result<mir::CanonicalMirCallableBindingsV1, Error> {
        if self.records.len() != self.source.len() {
            return Err(Error::IncompleteConstructors {
                expected: self.source.len(),
                actual: self.records.len(),
            });
        }

        Ok(mir::CanonicalMirCallableBindingsV1::try_new(self.records)?)
    }
}
