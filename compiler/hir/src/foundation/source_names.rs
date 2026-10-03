//! Source-facing labels recovered from the original declaration records.

use scoop_identity::{
    CallableOwner, CallableTemplateOrigin, CallableTemplateOwner, DeclarationName,
    LexicalCallableRole, NominalDeclarationOwner, PropertyOwner,
};

use super::*;
use crate::SourceContextNames;

impl CanonicalHirFoundation {
    pub(crate) fn source_context_names(
        &self,
        context: &SourceContextKey,
    ) -> Option<SourceContextNames> {
        match context {
            SourceContextKey::File { .. } => Some(SourceContextNames::default()),
            SourceContextKey::Nominal { owner, .. } => Some(SourceContextNames {
                function: String::new(),
                type_name: self.nominal_name(*owner)?,
            }),
            SourceContextKey::Property { owner, .. } => self.property_names(*owner),
            SourceContextKey::Initialization { unit, .. } => self.initialization_names(*unit),
            SourceContextKey::Callable { owner, .. } => {
                let template = match owner {
                    CallableOwner::Function(id) => CallableTemplateOwner::Function(*id),
                    CallableOwner::GenericTemplate(id) => {
                        CallableTemplateOwner::GenericFunction(*id)
                    }
                    CallableOwner::Constructor(id) => CallableTemplateOwner::Constructor(*id),
                    CallableOwner::Accessor(id) => CallableTemplateOwner::Accessor(*id),
                    CallableOwner::Generated(id) => CallableTemplateOwner::Generated(*id),
                    CallableOwner::Application(id) => {
                        return self
                            .origin_names(record_key(&self.callable_applications, *id)?.origin());
                    }
                };
                self.callable_names(template)
            }
        }
    }

    fn nominal_name(&self, owner: NominalDeclarationOwner) -> Option<String> {
        let key = match owner {
            NominalDeclarationOwner::Concrete(id) => record_key(&self.types, id),
            NominalDeclarationOwner::GenericTemplate(id) => record_key(&self.generic_types, id),
        }?;
        Some(declaration_name(key))
    }

    fn declaration_names(&self, key: &SourceDeclarationKey) -> Option<SourceContextNames> {
        let owner = key
            .owners()
            .owners()
            .iter()
            .rev()
            .find_map(|owner| match owner {
                DefinitionOwnerAtom::Type(id) => Some(NominalDeclarationOwner::Concrete(*id)),
                DefinitionOwnerAtom::GenericType(id) => {
                    Some(NominalDeclarationOwner::GenericTemplate(*id))
                }
                _ => None,
            });
        Some(SourceContextNames {
            function: declaration_name(key),
            type_name: match owner {
                Some(owner) => self.nominal_name(owner)?,
                None => String::new(),
            },
        })
    }

    fn property_names(&self, owner: PropertyOwner) -> Option<SourceContextNames> {
        let key = match owner {
            PropertyOwner::Property(id) => record_key(&self.properties, id),
            PropertyOwner::ExtensionProperty(id) => record_key(&self.extension_properties, id),
        }?;
        self.declaration_names(key)
    }

    fn initialization_names(
        &self,
        unit: PersistentInitializationUnitId,
    ) -> Option<SourceContextNames> {
        match record_key(&self.initialization_units, unit)? {
            InitializationUnitKey::TopLevelProperty(id) => {
                self.property_names(PropertyOwner::Property(*id))
            }
            InitializationUnitKey::ExtensionProperty(id)
            | InitializationUnitKey::GenericDelegatedExtensionApplication {
                property: id, ..
            } => self.property_names(PropertyOwner::ExtensionProperty(*id)),
            InitializationUnitKey::Object(id) | InitializationUnitKey::Companion(id) => {
                Some(SourceContextNames {
                    function: String::new(),
                    type_name: self.nominal_name(NominalDeclarationOwner::Concrete(*id))?,
                })
            }
        }
    }

    fn origin_names(&self, origin: CallableTemplateOrigin) -> Option<SourceContextNames> {
        self.callable_names(match origin {
            CallableTemplateOrigin::Function(id) => CallableTemplateOwner::Function(id),
            CallableTemplateOrigin::GenericFunction(id) => {
                CallableTemplateOwner::GenericFunction(id)
            }
            CallableTemplateOrigin::Constructor(id) => CallableTemplateOwner::Constructor(id),
            CallableTemplateOrigin::Accessor(id) => CallableTemplateOwner::Accessor(id),
            CallableTemplateOrigin::VariantConstructor(id) => {
                CallableTemplateOwner::VariantConstructor(id)
            }
        })
    }

    fn callable_names(&self, callable: CallableTemplateOwner) -> Option<SourceContextNames> {
        let declaration = match callable {
            CallableTemplateOwner::Function(id) => record_key(&self.functions, id),
            CallableTemplateOwner::GenericFunction(id) => record_key(&self.generic_functions, id),
            CallableTemplateOwner::Constructor(id) => record_key(&self.constructors, id),
            CallableTemplateOwner::Accessor(id) => {
                return self.property_names(record_key(&self.property_accessors, id)?.owner());
            }
            CallableTemplateOwner::VariantConstructor(id) => {
                let key = record_key(&self.enum_variants, id)?;
                return Some(SourceContextNames {
                    function: "<init>".to_owned(),
                    type_name: self.nominal_name(key.source_owner()?)?,
                });
            }
            CallableTemplateOwner::Generated(id) => {
                return self.generated_names(record_key(&self.generated_callables, id)?);
            }
        }?;
        self.declaration_names(declaration)
    }

    fn generated_names(&self, key: &GeneratedCallableKey) -> Option<SourceContextNames> {
        match key {
            GeneratedCallableKey::Lexical { parent, role, .. } => {
                let mut names = self.callable_names(parent.template())?;
                names.function = match role {
                    LexicalCallableRole::LambdaBody => "<lambda>",
                    LexicalCallableRole::AnonymousFunctionBody => "<anonymous>",
                }
                .to_owned();
                Some(names)
            }
            GeneratedCallableKey::Initialization { unit, .. } => self.initialization_names(*unit),
            GeneratedCallableKey::CallableReferenceInvoke { parent, .. } => {
                self.callable_names(parent.template())
            }
            GeneratedCallableKey::StaticNoGcCallbackStorageBridge { source, .. }
            | GeneratedCallableKey::CoroutineDriver {
                source_callable: source,
            }
            | GeneratedCallableKey::CoroutineAdapter {
                source_callable: source,
                ..
            }
            | GeneratedCallableKey::DispatchAdjust { target: source, .. } => {
                self.callable_names(source.template())
            }
            GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } => {
                self.callable_names(CallableTemplateOwner::Constructor(*constructor))
            }
            // These machine helpers do not introduce a source lexical scope.
            GeneratedCallableKey::DerivedEquality { .. }
            | GeneratedCallableKey::FunctionAdapter { .. }
            | GeneratedCallableKey::DynamicFunctionAdapter { .. }
            | GeneratedCallableKey::ForeignCallbackManagedAdapter { .. }
            | GeneratedCallableKey::CoroutineStart { .. }
            | GeneratedCallableKey::FunctionBridge { .. }
            | GeneratedCallableKey::BoxingAdjust { .. } => Some(SourceContextNames::default()),
        }
    }
}

fn declaration_name(key: &SourceDeclarationKey) -> String {
    match key.name() {
        DeclarationName::Named(name) => name.as_str().to_owned(),
        DeclarationName::Constructor => "<init>".to_owned(),
    }
}

fn record_key<I: PersistentId, K>(records: &[CborIdentityRecord<I, K>], id: I) -> Option<&K> {
    records
        .iter()
        .find(|record| record.id() == id)
        .map(CborIdentityRecord::key)
}
