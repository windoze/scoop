use la_arena::Arena;
use scoop_hir as hir;
use scoop_identity::{CallableTemplateOwner, StructuralDefinitionPath};

#[derive(Clone)]
pub(crate) enum DefaultScopeRoot {
    Declared(hir::LexicalDefinitionRoot),
    Resolved(CallableTemplateOwner),
}

#[derive(Clone)]
pub(crate) struct PendingDefaultLocalScope {
    pub(crate) definition_root: DefaultScopeRoot,
    pub(crate) definition_path: StructuralDefinitionPath,
    pub(crate) values: Vec<hir::DefaultLocalValueDefinition>,
}

pub(crate) fn finish_default_local_scopes(
    scopes: Arena<PendingDefaultLocalScope>,
    functions: &hir::HirFunctionIdentities,
    accessors: &hir::HirPropertyAccessorIdentities,
    constructors: &hir::HirConstructorIdentities,
    variants: &hir::HirEnumMemberIdentities,
) -> Arena<hir::DefaultLocalValueScope> {
    scopes
        .into_iter()
        .map(|(_, scope)| {
            let definition_root = match scope.definition_root {
                DefaultScopeRoot::Resolved(root) => root,
                DefaultScopeRoot::Declared(root) => match root {
                    hir::LexicalDefinitionRoot::Function(function) => match &functions[function] {
                        hir::HirFunctionIdentity::Source(
                            hir::HirSourceFunctionIdentity::Plain(record),
                        ) => CallableTemplateOwner::Function(record.id()),
                        hir::HirFunctionIdentity::Source(
                            hir::HirSourceFunctionIdentity::Generic(record),
                        ) => CallableTemplateOwner::GenericFunction(record.id()),
                        hir::HirFunctionIdentity::PropertyAccessor(accessor) => {
                            let identity = match accessor {
                                hir::HirPropertyAccessorFunction::Getter(getter) => {
                                    accessors.get_getter(*getter)
                                }
                                hir::HirPropertyAccessorFunction::Setter(setter) => {
                                    accessors.get_setter(*setter)
                                }
                            }
                            .expect("a source accessor retains its original identity");
                            CallableTemplateOwner::Accessor(identity.id())
                        }
                        hir::HirFunctionIdentity::LexicalGenerated(record)
                        | hir::HirFunctionIdentity::Initialization { record, .. } => {
                            CallableTemplateOwner::Generated(record.id())
                        }
                        hir::HirFunctionIdentity::DerivedEquality(_) => {
                            unreachable!("derived equality has no source parameter defaults")
                        }
                    },
                    hir::LexicalDefinitionRoot::ClassConstructor(constructor) => {
                        CallableTemplateOwner::Constructor(
                            constructors[constructor]
                                .source_record()
                                .expect("source defaults belong to source constructors")
                                .id(),
                        )
                    }
                    hir::LexicalDefinitionRoot::StructConstructor(constructor) => {
                        CallableTemplateOwner::Constructor(constructors[constructor].id())
                    }
                    hir::LexicalDefinitionRoot::VariantConstructor(variant) => {
                        CallableTemplateOwner::VariantConstructor(variants[variant].id())
                    }
                },
            };
            hir::DefaultLocalValueScope {
                definition_root,
                definition_path: scope.definition_path,
                values: scope.values,
            }
        })
        .collect()
}
