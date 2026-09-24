use super::*;
use std::collections::BTreeMap;

pub(super) struct Producer<'a, 'm> {
    source: BTreeMap<PersistentConstructorId, &'a hir::CallableDeclarationRecordV1>,
    input: &'a mir::SingleConeStrongMirInput,
    authority: mir::MirCallableBridgeAuthority<'a>,
    records: Vec<mir::ParamFreeMirCallableBindingV1>,
    meter: &'m mut BudgetMeter,
}
impl<'a, 'm> Producer<'a, 'm> {
    pub(super) fn new(
        public: &'a hir::CrossConeHirInterfaceSectionV1,
        input: &'a mir::SingleConeStrongMirInput,
        identities: &'a ValidatedIdentityGraph,
        types: &'a dyn mir::MirTypeBridgeTypeLookupV1,
        meter: &'m mut BudgetMeter,
    ) -> Result<Self, Error> {
        let source = hir::select_param_free_source_constructors(
            input.module().cone,
            public,
            identities,
            meter,
        )?;
        let mut records = Vec::new();
        meter.charge_owned_bytes(
            (source.len() as u64)
                .saturating_mul(std::mem::size_of::<mir::ParamFreeMirCallableBindingV1>() as u64),
            &WirePath::root(),
        )?;
        meter.try_reserve_collection_slots(&mut records, source.len(), &WirePath::root())?;
        Ok(Self {
            source,
            input,
            authority: mir::MirCallableBridgeAuthority {
                identities,
                foundation: input.foundation(),
                types,
            },
            records,
            meter,
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
        self.meter.charge_nodes(1, &WirePath::root())?;
        self.meter.charge_work(
            u64::from(self.source.len().checked_ilog2().unwrap_or(0)) + 1,
            &WirePath::root(),
        )?;
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

    pub(super) fn signature_cost(&mut self, types: usize, parameters: usize) -> Result<(), Error> {
        self.meter
            .charge_collection_slots((parameters as u64).saturating_mul(2), &WirePath::root())?;
        self.meter.charge_work(
            (types as u64).saturating_add(parameters as u64 * 3 + 1),
            &WirePath::root(),
        )?;
        self.meter.charge_owned_bytes(
            (parameters as u64).saturating_mul(
                2 * std::mem::size_of::<scoop_identity::PersistentExactTypeId>() as u64,
            ),
            &WirePath::root(),
        )?;
        Ok(())
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
        let type_work = u64::from(
            self.authority
                .types
                .record_count()
                .checked_ilog2()
                .unwrap_or(0),
        ) + 1;
        self.meter.charge_work(
            (signatures.len() as u64)
                .saturating_add(u64::from(roots.len().checked_ilog2().unwrap_or(0)) + 1)
                .saturating_add((semantic.parameters().len() as u64 * 2 + 8) * type_work + 32),
            &WirePath::root(),
        )?;
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
        self.meter.charge_work(
            (self.records.len() as u64)
                .saturating_mul(u64::from(self.records.len().checked_ilog2().unwrap_or(0)) + 1),
            &WirePath::root(),
        )?;
        Ok(mir::CanonicalMirCallableBindingsV1::try_new(self.records)?)
    }
}
