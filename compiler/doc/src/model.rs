//! Local render inputs. Routing keys are presentation addresses, not language IDs.

use serde::{Deserialize, Serialize};

mod keys;
mod text;
pub use keys::{ConeKey, SourceKey, SymbolKey};
pub use text::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConeInput {
    pub key: ConeKey,
    pub name: String,
    pub executable: bool,
    /// Only sources whose text is available. Locations may refer to absent sources.
    pub sources: Vec<SourceInput>,
    pub items: Vec<ApiItem>,
    pub bindings: Vec<ExportBinding>,
    pub warnings: Vec<DocWarning>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceInput {
    pub key: SourceKey,
    pub path: String,
    pub display_name: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiItem {
    pub key: SymbolKey,
    /// Other actual identities that address this same source declaration
    /// (for example an object's singleton value).
    pub alternate_keys: Vec<SymbolKey>,
    pub parent: Option<SymbolKey>,
    pub package: Vec<String>,
    pub kind: ApiKind,
    pub name: String,
    pub visibility: Visibility,
    pub public: bool,
    pub hidden: bool,
    pub aliases: Vec<String>,
    pub synthesized: bool,
    pub signature: Vec<SignatureToken>,
    pub receiver: Option<SignatureToken>,
    pub equality_requirements: Option<EqualityRequirements>,
    pub location: SourceLocation,
    pub comment: Option<Comment>,
    pub property: Option<PropertyCapability>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignatureToken {
    pub text: String,
    pub link: Option<SymbolKey>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EqualityRequirements {
    /// Actual field/payload types in the declaration's original binder scope.
    pub components: Vec<SignatureToken>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PropertyCapability {
    pub getter: AccessorCapability,
    pub setter: Option<AccessorCapability>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccessorCapability {
    pub key: SymbolKey,
    pub visibility: Visibility,
    pub hidden: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportBinding {
    pub package: Vec<String>,
    pub name: String,
    pub target: SymbolKey,
    pub reexport: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Internal,
    Private,
    Protected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiKind {
    Class,
    Struct,
    Enum,
    Interface,
    Object,
    Annotation,
    TypeAlias,
    Function,
    Constructor,
    Property,
    Field,
    Variant,
    Parameter,
    TypeParameter,
    ContextParameter,
    Getter,
    Setter,
}

impl ConeInput {
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Read an input only when rendering actually needs it. Cache-hit
    /// decisions belong to the caller and do not decode existing results.
    pub fn from_json(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }

    /// Filtering consumes the checked effective access facts. Parent edges
    /// prevent a hidden or inaccessible container from leaking its children.
    pub fn visible_items(&self, private: bool) -> Vec<&ApiItem> {
        let private = private || self.executable;
        let candidates = self
            .items
            .iter()
            .map(|item| (&item.key, item))
            .collect::<std::collections::HashMap<_, _>>();
        let visible = |item: &ApiItem| {
            let mut current = Some(item);
            while let Some(entry) = current {
                if entry.hidden || (!private && !entry.public) {
                    return false;
                }
                current = entry.parent.as_ref().map(|parent| candidates[parent]);
            }
            true
        };
        self.items.iter().filter(|item| visible(item)).collect()
    }
}
