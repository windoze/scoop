//! Transient local emission components, not a persistent identity or wire format.

use crate::{Lowerer, Owner, namespace::TopLevelLookupLayer};
use scoop_hir as hir;

#[derive(Clone, Copy)]
pub(crate) enum LocalLinkRole {
    GlobalStorage,
    TopLevelInitialization,
    ExtensionInitialization(hir::TypeId),
    SingletonInitialization,
    Nominal(LocalNominalLinkRole),
    Callable(LocalCallableLinkRole),
}

/// Closed source-nominal roles. An object's compiler-owned backing class is
/// deliberately distinct from the object declaration with the same display
/// name.
#[derive(Clone, Copy)]
pub(crate) enum LocalNominalLinkRole {
    Struct,
    Enum,
    Class,
    Interface,
    Object,
    ObjectBackingClass,
}

#[derive(Clone, Copy)]
pub(crate) enum LocalCallableScope {
    TopLevel(TopLevelCallableScope),
    Member(Owner),
    SourceLocal,
}

#[derive(Clone, Copy)]
pub(crate) enum TopLevelCallableScope {
    Package,
    FilePrivate,
}

#[derive(Clone, Copy)]
pub(crate) enum LocalCallableReceiver {
    Ordinary,
    /// A source extension function. Its resolved receiver is the first ABI
    /// parameter and participates in ordinary overload encoding.
    ExtensionDeclaration,
    /// An extension property accessor, allocated only after the property
    /// receiver has a complete typed identity.
    ExtensionProperty(hir::TypeId),
}

#[derive(Clone, Copy)]
pub(crate) enum LocalCallableLinkRole {
    Function(LocalCallableReceiver),
    PropertyGetter(LocalCallableReceiver),
    PropertySetter(LocalCallableReceiver),
    InitializationBody,
    InitializationEnsure,
    LocalFunction(u32),
    Lambda(u32),
    AnonymousFunction(u32),
    StructuralDerivedEquality(hir::TypeId),
}

/// Every field has a kind tag and a UTF-8 byte length. Package segments,
/// nominal owner edges and declaration roles cannot alias one another.
fn field(output: &mut String, tag: char, value: &str) {
    use std::fmt::Write;
    write!(output, "{tag}{}:{value}", value.len()).expect("writing a String is infallible");
}

impl Lowerer {
    pub(crate) fn local_link_component(
        &self,
        file: usize,
        owner: Option<Owner>,
        name: &str,
        private: bool,
        role: LocalLinkRole,
    ) -> String {
        let mut key = String::from("$local$");
        let mut package = String::new();
        match self.top_level_namespaces.source_namespace(file) {
            TopLevelLookupLayer::CorePrelude => field(&mut key, 'c', ""),
            TopLevelLookupLayer::CurrentPackage(id) => {
                field(&mut key, 'u', "");
                for segment in self.top_level_namespaces.package_segments(id) {
                    field(&mut package, 's', segment);
                }
            }
        }
        field(&mut key, 'p', &package);
        if let Some(owner) = owner {
            self.append_link_owner(&mut key, owner);
        }
        if private {
            self.append_link_source(&mut key, file);
        }
        match role {
            LocalLinkRole::GlobalStorage => field(&mut key, 'r', "global"),
            LocalLinkRole::TopLevelInitialization => field(&mut key, 'r', "top-level"),
            LocalLinkRole::ExtensionInitialization(receiver) => {
                field(&mut key, 'r', "extension");
                field(&mut key, 't', &self.local_link_type(receiver));
            }
            LocalLinkRole::SingletonInitialization => field(&mut key, 'r', "singleton"),
            LocalLinkRole::Nominal(role) => self.append_nominal_link_role(&mut key, role),
            LocalLinkRole::Callable(role) => self.append_callable_link_role(&mut key, role),
        }
        field(&mut key, 'n', name);
        key
    }

    pub(crate) fn local_nominal_link_stem(
        &self,
        file: usize,
        owner: Option<Owner>,
        name: &str,
        private: bool,
        role: LocalNominalLinkRole,
    ) -> hir::NominalLinkStem {
        hir::NominalLinkStem::from_session_local_encoding(self.local_link_component(
            file,
            owner,
            name,
            private,
            LocalLinkRole::Nominal(role),
        ))
    }

    fn append_nominal_link_role(&self, key: &mut String, role: LocalNominalLinkRole) {
        let role = match role {
            LocalNominalLinkRole::Struct => "struct",
            LocalNominalLinkRole::Enum => "enum",
            LocalNominalLinkRole::Class => "class",
            LocalNominalLinkRole::Interface => "interface",
            LocalNominalLinkRole::Object => "object",
            LocalNominalLinkRole::ObjectBackingClass => "object-backing-class",
        };
        field(key, 'r', role);
    }

    pub(crate) fn local_callable_link_stem(
        &self,
        file: usize,
        scope: LocalCallableScope,
        name: &str,
        role: LocalCallableLinkRole,
    ) -> hir::CallableLinkStem {
        let (owner, private) = match scope {
            LocalCallableScope::TopLevel(TopLevelCallableScope::Package) => (None, false),
            LocalCallableScope::TopLevel(TopLevelCallableScope::FilePrivate)
            | LocalCallableScope::SourceLocal => (None, true),
            LocalCallableScope::Member(owner) => (Some(owner), false),
        };
        hir::CallableLinkStem::from_session_local_encoding(self.local_link_component(
            file,
            owner,
            name,
            private,
            LocalLinkRole::Callable(role),
        ))
    }

    fn append_callable_link_role(&self, key: &mut String, role: LocalCallableLinkRole) {
        match role {
            LocalCallableLinkRole::Function(receiver) => {
                field(key, 'r', "function");
                self.append_callable_receiver(key, receiver);
            }
            LocalCallableLinkRole::PropertyGetter(receiver) => {
                field(key, 'r', "getter");
                self.append_callable_receiver(key, receiver);
            }
            LocalCallableLinkRole::PropertySetter(receiver) => {
                field(key, 'r', "setter");
                self.append_callable_receiver(key, receiver);
            }
            LocalCallableLinkRole::InitializationBody => field(key, 'r', "init-body"),
            LocalCallableLinkRole::InitializationEnsure => field(key, 'r', "init-ensure"),
            LocalCallableLinkRole::LocalFunction(index) => {
                field(key, 'r', "local-function");
                field(key, 'i', &index.to_string());
            }
            LocalCallableLinkRole::Lambda(index) => {
                field(key, 'r', "lambda");
                field(key, 'i', &index.to_string());
            }
            LocalCallableLinkRole::AnonymousFunction(index) => {
                field(key, 'r', "anonymous-function");
                field(key, 'i', &index.to_string());
            }
            LocalCallableLinkRole::StructuralDerivedEquality(owner) => {
                field(key, 'r', "structural-derived-equality");
                field(key, 't', &self.local_link_type(owner));
            }
        }
    }

    fn append_callable_receiver(&self, key: &mut String, receiver: LocalCallableReceiver) {
        match receiver {
            LocalCallableReceiver::Ordinary => field(key, 'q', "ordinary"),
            LocalCallableReceiver::ExtensionDeclaration => field(key, 'q', "extension"),
            LocalCallableReceiver::ExtensionProperty(ty) => {
                field(key, 'q', "extension");
                field(key, 't', &self.local_link_type(ty));
            }
        }
    }

    fn append_link_source(&self, key: &mut String, file: usize) {
        let source = self.visibility_file(file).source;
        field(key, 'c', &source.cone().to_string());
        field(key, 'f', source.logical_path().as_str());
    }

    fn link_owner_parts(
        &self,
        owner: Owner,
    ) -> (&str, Option<hir::NominalOwner>, usize, bool, char) {
        let (name, parent, file, visibility, tag) = match owner {
            Owner::Class(id) => (
                &self.classes[id].name,
                self.classes[id].owner,
                self.class_files[&id],
                self.classes[id].access.declared,
                'C',
            ),
            Owner::Interface(id) => (
                &self.interfaces[id].name,
                self.interfaces[id].owner,
                self.interface_files[&id],
                self.interfaces[id].access.declared,
                'I',
            ),
            Owner::Struct(id) => (
                &self.structs[id].name,
                self.structs[id].owner,
                self.struct_files[&id],
                self.structs[id].access.declared,
                'S',
            ),
            Owner::Enum(id) => (
                &self.enums[id].name,
                self.enums[id].owner,
                self.enum_files[&id],
                self.enums[id].access.declared,
                'E',
            ),
            Owner::Object(id) => (
                &self.objects[id].name,
                self.objects[id].owner,
                self.object_files[&id],
                self.objects[id].access.declared,
                'O',
            ),
        };
        (
            name,
            parent,
            file,
            visibility == hir::DeclaredVisibility::Private,
            tag,
        )
    }

    fn append_link_owner(&self, key: &mut String, owner: Owner) {
        let (name, parent, file, private, tag) = self.link_owner_parts(owner);
        if let Some(parent) = parent {
            self.append_link_owner(key, Owner::from_nominal_owner(parent));
        }
        let mut edge = String::new();
        field(&mut edge, 'n', name);
        if private && parent.is_none() {
            self.append_link_source(&mut edge, file);
        }
        field(key, tag, &edge);
    }

    fn local_link_nominal(&self, stem: &hir::NominalLinkStem, arguments: &[hir::TypeId]) -> String {
        let mut key = String::new();
        field(&mut key, 'd', stem.as_str());
        for argument in arguments {
            field(&mut key, 'a', &self.local_link_type(*argument));
        }
        key
    }

    fn local_link_type(&self, ty: hir::TypeId) -> String {
        let mut key = String::new();
        match &self.types[ty] {
            hir::Type::Unit => field(&mut key, 'v', "Unit"),
            hir::Type::Integer(kind) => field(&mut key, 'v', kind.canonical_name()),
            hir::Type::Boolean => field(&mut key, 'v', "Boolean"),
            hir::Type::String => field(&mut key, 'v', "String"),
            hir::Type::Any => field(&mut key, 'v', "Any"),
            hir::Type::Struct(id) => {
                let application = &self.struct_applications[*id];
                return self.local_link_nominal(
                    &self.structs[application.template].link_stem,
                    &application.arguments,
                );
            }
            hir::Type::Class(id) => {
                let application = &self.class_applications[*id];
                return self.local_link_nominal(
                    &self.classes[application.template].link_stem,
                    &application.arguments,
                );
            }
            hir::Type::Interface(id) => {
                let application = &self.interface_applications[*id];
                return self.local_link_nominal(
                    &self.interfaces[application.template].link_stem,
                    &application.arguments,
                );
            }
            hir::Type::Enum(id) => {
                let application = &self.enum_applications[*id];
                return self.local_link_nominal(
                    &self.enums[application.template].link_stem,
                    &application.arguments,
                );
            }
            hir::Type::Tuple(elements) => {
                let mut fields = String::new();
                for element in elements {
                    field(&mut fields, 'e', &self.local_link_type(*element));
                }
                field(&mut key, 'T', &fields);
            }
            hir::Type::Ptr(pointee) => field(&mut key, 'P', &self.local_link_type(*pointee)),
            hir::Type::Function(id) | hir::Type::FunPtr(id) => {
                let signature = &self.function_types[*id];
                let mut fields = String::new();
                field(
                    &mut fields,
                    's',
                    if signature.is_suspend { "1" } else { "0" },
                );
                for parameter in &signature.parameter_types {
                    field(&mut fields, 'p', &self.local_link_type(*parameter));
                }
                field(
                    &mut fields,
                    'r',
                    &self.local_link_type(signature.return_type),
                );
                field(
                    &mut key,
                    if matches!(self.types[ty], hir::Type::FunPtr(_)) {
                        'N'
                    } else {
                        'F'
                    },
                    &fields,
                );
            }
            hir::Type::Param(parameter) => field(&mut key, 'V', &parameter.into_raw().to_string()),
        }
        key
    }
}

#[cfg(test)]
mod tests;
