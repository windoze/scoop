use super::*;
use scoop_identity::PropertyOwner;

impl Query<'_, '_, '_> {
    pub(super) fn callable<'t>(&mut self, declaration: Declaration) -> Result<Access<'t>, Error> {
        self.meter.charge_edges(1, &self.path)?;
        let (subject, origin) = match declaration {
            Declaration::Function(id) => {
                (Subject::Function(id), CallableTemplateOrigin::Function(id))
            }
            Declaration::GenericFunction(id) => (
                Subject::GenericFunction(id),
                CallableTemplateOrigin::GenericFunction(id),
            ),
            Declaration::PropertyAccessor(id) => return self.accessor(id),
            Declaration::Generated(id) => return self.generated_callable(id),
        };
        let key = self.declaration(subject)?;
        if matches!(key.scope(), DeclarationScope::LexicalScoped { .. }) {
            if !matches!(
                key.owners().owners().last(),
                Some(
                    DefinitionOwnerAtom::Function(_)
                        | DefinitionOwnerAtom::GenericFunction(_)
                        | DefinitionOwnerAtom::Constructor(_)
                        | DefinitionOwnerAtom::PropertyAccessor(_)
                        | DefinitionOwnerAtom::GeneratedCallable(_)
                        | DefinitionOwnerAtom::EnumVariant(_)
                )
            ) {
                return Err(Error::DeclarationScope(subject));
            }
            Ok(Access::Nested(Nested::LocalFunction(origin)))
        } else {
            self.nominal_callable_scope(subject, key)?;
            Ok(Access::Declaration(subject))
        }
    }
    fn accessor<'t>(&mut self, id: PersistentPropertyAccessorId) -> Result<Access<'t>, Error> {
        let canonical = self.foundation.foundation.as_canonical();
        let subject = Subject::PropertyAccessor(id);
        let key = self.key(canonical.type_source_accessor_records(), id, || {
            Error::MissingDeclaration(subject)
        })?;
        self.meter.charge_edges(1, &self.path)?;
        let property = match key.owner() {
            PropertyOwner::Property(id) => Subject::Property(id),
            PropertyOwner::ExtensionProperty(id) => Subject::ExtensionProperty(id),
        };
        let key = self.declaration(property)?;
        self.nominal_callable_scope(subject, key)?;
        Ok(Access::Declaration(subject))
    }
    fn nominal_callable_scope(
        &mut self,
        subject: Subject,
        key: &SourceDeclarationKey,
    ) -> Result<(), Error> {
        let owners = key.owners().owners();
        self.meter
            .charge_work(owners.len() as u64 + 1, &self.path)?;
        if matches!(key.scope(), DeclarationScope::LexicalScoped { .. })
            || owners.iter().any(|owner| {
                !matches!(
                    owner,
                    DefinitionOwnerAtom::Type(_) | DefinitionOwnerAtom::GenericType(_)
                )
            })
        {
            return Err(Error::DeclarationScope(subject));
        }
        Ok(())
    }
    fn generated_callable<'t>(
        &mut self,
        id: PersistentGeneratedCallableId,
    ) -> Result<Access<'t>, Error> {
        let canonical = self.foundation.foundation.as_canonical();
        let key = self.key(
            canonical.type_source_generated_callable_records(),
            id,
            || Error::MissingCallable(id),
        )?;
        let identity = match key {
            GeneratedCallableKey::Lexical {
                role: LexicalCallableRole::LambdaBody,
                ..
            } => Nested::Lambda(id),
            GeneratedCallableKey::Lexical {
                role: LexicalCallableRole::AnonymousFunctionBody,
                ..
            } => Nested::AnonymousFunction(id),
            GeneratedCallableKey::CallableReferenceInvoke { .. } => Nested::CallableReference(id),
            _ => return Err(Error::CallableRole(Declaration::Generated(id))),
        };
        Ok(Access::Nested(identity))
    }
}
