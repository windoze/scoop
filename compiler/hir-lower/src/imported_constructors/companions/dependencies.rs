use super::*;
use scoop_identity::{
    CallableOwner, PersistentSourceContextId, SignatureTypeKey, SourceContextKey,
};
use scoop_wire::{WireError, WirePath};

impl Lowerer {
    pub(in crate::imported_constructors) fn complete_companion_dependencies(
        &mut self,
        constructor: hir::ImportedConstructorTemplateId,
        source: &PreparedImportedConstructor,
    ) -> Result<(), String> {
        let Some((companion, _)) = self
            .imported_companion_templates
            .iter()
            .find(|(_, template)| template.constructor == constructor)
        else {
            return Ok(());
        };
        let context = PersistentSourceContextId::from_key(&SourceContextKey::Callable {
            source: source.source.definition_origin().origin().source().clone(),
            owner: CallableOwner::Constructor(source.signature.declaration),
        })
        .map_err(|error| error.to_string())?;
        let mut reads = DirectSingletonReads {
            context,
            types: Vec::new(),
        };
        source
            .initialization
            .visit_direct_references(&mut reads, &WirePath::root())
            .map_err(|error| error.to_string())?;
        reads.types.sort_unstable();
        reads.types.dedup();
        let dependencies = reads
            .types
            .into_iter()
            .map(|ty| self.imported_generic_type(ty, &source.bindings))
            .collect::<Result<_, _>>()?;
        self.imported_companion_templates[companion].dependencies = dependencies;
        Ok(())
    }
}

struct DirectSingletonReads<'a> {
    context: PersistentSourceContextId,
    types: Vec<&'a SignatureTypeKey>,
}

impl<'a> hir::DefaultBodyReferenceVisitorV1<'a> for DirectSingletonReads<'a> {
    type Error = WireError;

    fn expression(
        &mut self,
        _index: u32,
        expression: &'a hir::DefaultExpressionV1,
        _path: &WirePath,
    ) -> Result<(), WireError> {
        // Defaults and nested callable bodies retain their own source context.
        if expression.definition_origin().origin().context() == self.context
            && matches!(
                expression.kind(),
                hir::DefaultExpressionKindV1::SingletonValue(_)
            )
            && matches!(
                expression.result_type(),
                SignatureTypeKey::NominalApplication { .. }
            )
        {
            self.types.push(expression.result_type());
        }
        Ok(())
    }

    fn reference(
        &mut self,
        _occurrence: hir::DefaultBodyReferenceOccurrenceV1<'a>,
        _path: &WirePath,
    ) -> Result<(), WireError> {
        Ok(())
    }
}
