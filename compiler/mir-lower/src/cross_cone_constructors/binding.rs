use super::*;
use std::collections::BTreeMap;

pub(super) struct Producer<'a> {
    source: BTreeMap<PersistentConstructorId, &'a hir::CallableDeclarationRecordV1>,
    input: &'a mir::ConeMirInput,
    authority: mir::MirCallableBridgeAuthority<'a>,
    records: Vec<mir::ParamFreeMirCallableBindingV1>,
}
impl<'a> Producer<'a> {
    pub(super) fn new(
        public: &'a hir::CrossConeHirInterfaceSectionV1,
        input: &'a mir::ConeMirInput,
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
            mir::MirCallableOriginV1,
        )>,
        Error,
    > {
        let (declaration, origin) = match materialization.template() {
            CallableTemplateOwner::Constructor(declaration) => (
                declaration,
                mir::MirCallableOriginV1::Constructor(declaration),
            ),
            CallableTemplateOwner::Generated(callable) => {
                let role = self
                    .authority
                    .identities
                    .canonical_key::<_, scoop_identity::GeneratedCallableKey>(callable)
                    .map_err(mir::MirCallableBridgeError::from)?;
                let scoop_identity::GeneratedCallableKey::ZeroArgumentConstructorAdapter {
                    constructor,
                } = *role
                else {
                    return Ok(None);
                };
                (
                    constructor,
                    mir::MirCallableOriginV1::Generated {
                        callable,
                        role: (*role).clone(),
                    },
                )
            }
            _ => return Ok(None),
        };
        let Some(&source) = self.source.get(&declaration) else {
            return Ok(None);
        };
        if materialization.context() != CallableMaterializationContext::NoSubstitution {
            return Err(Error::OdrRequired(declaration));
        }
        Ok(Some((declaration, source, origin)))
    }

    pub(super) fn record(
        &mut self,
        declaration: PersistentConstructorId,
        source: &hir::CallableDeclarationRecordV1,
        origin: mir::MirCallableOriginV1,
        semantic: ExactCallableSignature,
        lowered: ExactCallableSignature,
        role: mir::MirCallableLoweringRoleV1,
    ) -> Result<(), Error> {
        let roots = self.input.materialization().callable_roots();
        let signatures = &self.input.module().meta.callable_signatures;

        let implementation = origin.implementation();
        let index = roots
            .binary_search_by(|root| {
                root.subject()
                    .compare_sort_key(mir::CallableSignatureSubject::Strong(
                        implementation.callable_owner(),
                    ))
            })
            .map_err(|_| Error::MissingMaterialization(declaration))?;
        let root = roots[index];
        let actual = signatures
            .get(root.subject())
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
                origin,
                implementation,
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
        let source_count = self
            .records
            .iter()
            .filter(|binding| matches!(binding.origin(), mir::MirCallableOriginV1::Constructor(_)))
            .count();
        if source_count != self.source.len() {
            return Err(Error::IncompleteConstructors {
                expected: self.source.len(),
                actual: source_count,
            });
        }

        Ok(mir::CanonicalMirCallableBindingsV1::try_new(self.records)?)
    }
}
