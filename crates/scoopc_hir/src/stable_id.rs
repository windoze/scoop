//! Migration facade for shared stable-id helpers.
//!
//! Foundational hash/key primitives live in `scoopc_ids`, and cone identity
//! lives in `scoop_project_model`. This facade keeps canonical type/effect
//! encoding and compiler semantic keys next to the current HIR/MIR/RTTI users
//! until those stages are split. Callers must feed semantic keys or canonical
//! text here instead of reusing `TypeStore::display()`, raw `Debug` output, or
//! path/span text as the authoritative identity input.

use std::{collections::HashMap, fmt};

use thiserror::Error;

use scoopc_types::{
    EFFECT_ROW_PARAM_DECL_FILE, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind,
    TypeParamType, TypeStore, UnionType, ValueTypeKind,
};

pub use scoop_project_model::StableConeKey;
pub use scoopc_ids::{
    AbiMangler, AbiSymbolKind, BodyVersionKey, CanonicalTextKey, PrivateSymbolMangler, SiteId,
    StableCallSiteKey, StableCanonicalKey, StableHashScope, StableLirCallableKey, StableSymbolKey,
    canonical_list, canonical_record, stable_digest, stable_dump_label, stable_hash64,
    stable_hash128_hex, stable_local_label, stable_rtti_type_id,
};

const MAX_CANONICAL_DEPTH: usize = 64;

/// Export-visible declaration namespaces kept distinct by stable-id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum StableDefNamespace {
    Type,
    Value,
    Fun,
    PropertyGetter,
    PropertySetter,
    ObjectInit,
    TopLevelInit,
    ExternGlobal,
    Interface,
}

impl StableDefNamespace {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Value => "value",
            Self::Fun => "fun",
            Self::PropertyGetter => "property_getter",
            Self::PropertySetter => "property_setter",
            Self::ObjectInit => "object_init",
            Self::TopLevelInit => "top_level_init",
            Self::ExternGlobal => "extern_global",
            Self::Interface => "interface",
        }
    }
}

/// Semantic declaration identity used by exported symbols and stable templates.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StableDefKey {
    cone: StableConeKey,
    namespace: StableDefNamespace,
    owner_path: String,
    declaration_kind: String,
    overload_signature_key: Option<String>,
}

impl StableDefKey {
    pub fn new(
        cone: StableConeKey,
        namespace: StableDefNamespace,
        owner_path: impl Into<String>,
        declaration_kind: impl Into<String>,
        overload_signature_key: Option<String>,
    ) -> Self {
        Self {
            cone,
            namespace,
            owner_path: owner_path.into(),
            declaration_kind: declaration_kind.into(),
            overload_signature_key,
        }
    }

    pub fn cone(&self) -> &StableConeKey {
        &self.cone
    }

    pub fn namespace(&self) -> StableDefNamespace {
        self.namespace
    }

    pub fn owner_path(&self) -> &str {
        &self.owner_path
    }

    pub fn declaration_kind(&self) -> &str {
        &self.declaration_kind
    }

    pub fn overload_signature_key(&self) -> Option<&str> {
        self.overload_signature_key.as_deref()
    }
}

impl StableCanonicalKey for StableDefKey {
    fn canonical_text(&self) -> String {
        let mut parts = vec![
            self.cone.canonical_text(),
            self.namespace.as_str().to_string(),
            self.owner_path.clone(),
            self.declaration_kind.clone(),
        ];
        if let Some(signature) = &self.overload_signature_key {
            parts.push(signature.clone());
        }
        canonical_record("def", parts)
    }
}

impl StableSymbolKey for StableDefKey {
    fn readable_path(&self) -> &str {
        &self.owner_path
    }
}

/// Semantic template identity that replaces exported uses of `TemplateKey`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StableTemplateKey {
    def: StableDefKey,
}

impl StableTemplateKey {
    pub fn new(def: StableDefKey) -> Self {
        Self { def }
    }

    pub fn def(&self) -> &StableDefKey {
        &self.def
    }
}

impl StableCanonicalKey for StableTemplateKey {
    fn canonical_text(&self) -> String {
        canonical_record("template", [self.def.canonical_text()])
    }
}

impl StableSymbolKey for StableTemplateKey {
    fn readable_path(&self) -> &str {
        self.def.owner_path()
    }
}

/// Stable identity for an effect-row parameter owned by a declaration.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StableEffectParamKey {
    owner: StableDefKey,
    ordinal: u32,
    name: String,
}

impl StableEffectParamKey {
    pub fn new(owner: StableDefKey, ordinal: u32, name: impl Into<String>) -> Self {
        Self {
            owner,
            ordinal,
            name: name.into(),
        }
    }

    pub fn owner(&self) -> &StableDefKey {
        &self.owner
    }

    pub fn ordinal(&self) -> u32 {
        self.ordinal
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl StableCanonicalKey for StableEffectParamKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "eff_param",
            [
                self.owner.canonical_text(),
                self.ordinal.to_string(),
                self.name.clone(),
            ],
        )
    }
}

/// Resolves synthetic effect-row type parameters to stable effect-param keys.
pub trait StableEffectParamResolver {
    fn resolve_effect_param(&self, param: &TypeParamType) -> Option<StableEffectParamKey>;
}

impl<F> StableEffectParamResolver for F
where
    F: Fn(&TypeParamType) -> Option<StableEffectParamKey>,
{
    fn resolve_effect_param(&self, param: &TypeParamType) -> Option<StableEffectParamKey> {
        self(param)
    }
}

impl StableEffectParamResolver for HashMap<TypeParamType, StableEffectParamKey> {
    fn resolve_effect_param(&self, param: &TypeParamType) -> Option<StableEffectParamKey> {
        self.get(param).cloned()
    }
}

/// Resolver for rows that are expected to contain only concrete effect terms.
#[derive(Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct NoEffectParamResolver;

impl StableEffectParamResolver for NoEffectParamResolver {
    fn resolve_effect_param(&self, _: &TypeParamType) -> Option<StableEffectParamKey> {
        None
    }
}

/// Stable effect-row term used by cross-stage keys and facts.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum EffectTerm {
    Concrete {
        type_key: CanonicalTextKey,
    },
    Param {
        owner: StableDefKey,
        ordinal: u32,
        name: String,
    },
}

impl EffectTerm {
    pub fn concrete(type_key: impl Into<String>) -> Self {
        Self::Concrete {
            type_key: CanonicalTextKey::new(type_key),
        }
    }

    pub fn param(owner: StableDefKey, ordinal: u32, name: impl Into<String>) -> Self {
        Self::Param {
            owner,
            ordinal,
            name: name.into(),
        }
    }

    pub fn concrete_type_key(&self) -> Option<&CanonicalTextKey> {
        match self {
            Self::Concrete { type_key } => Some(type_key),
            Self::Param { .. } => None,
        }
    }

    pub fn param_key(&self) -> Option<StableEffectParamKey> {
        match self {
            Self::Param {
                owner,
                ordinal,
                name,
            } => Some(StableEffectParamKey::new(
                owner.clone(),
                *ordinal,
                name.clone(),
            )),
            Self::Concrete { .. } => None,
        }
    }
}

impl StableCanonicalKey for EffectTerm {
    fn canonical_text(&self) -> String {
        match self {
            Self::Concrete { type_key } => type_key.as_str().to_string(),
            Self::Param {
                owner,
                ordinal,
                name,
            } => StableEffectParamKey::new(owner.clone(), *ordinal, name.clone()).canonical_text(),
        }
    }
}

/// Stable representation of an effect row for semantic keys and fact handoff.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct EffectRowTemplate {
    terms: Vec<EffectTerm>,
    closed: bool,
}

impl EffectRowTemplate {
    pub fn new(mut terms: Vec<EffectTerm>, closed: bool) -> Self {
        terms.sort_by_key(|term| term.canonical_text());
        terms.dedup_by(|lhs, rhs| lhs.canonical_text() == rhs.canonical_text());
        Self { terms, closed }
    }

    pub fn pure() -> Self {
        Self::new(Vec::new(), false)
    }

    pub fn pure_closed() -> Self {
        Self::new(Vec::new(), true)
    }

    pub fn from_canonical_text(text: impl Into<String>) -> Result<Self, CanonicalEncodingError> {
        let text = text.into();
        let (row_text, closed) = match text.strip_suffix('!') {
            Some(open) => (open, true),
            None => (text.as_str(), false),
        };
        let Some(body) = row_text
            .strip_prefix("E(")
            .and_then(|rest| rest.strip_suffix(')'))
        else {
            return Err(CanonicalEncodingError::InvalidEffectRowTemplateText { text });
        };
        if body.is_empty() {
            return Ok(Self::new(Vec::new(), closed));
        }
        let terms = split_top_level_effect_terms(body)
            .into_iter()
            .map(EffectTerm::concrete)
            .collect();
        Ok(Self::new(terms, closed))
    }

    pub fn from_effect_row<R, E>(
        types: &TypeStore,
        row: &EffectRow,
        type_params: &R,
        effect_params: &E,
        closed: bool,
    ) -> Result<Self, CanonicalEncodingError>
    where
        R: StableTypeParamResolver + ?Sized,
        E: StableEffectParamResolver + ?Sized,
    {
        let mut encoder = CanonicalEncoder::new(types, type_params);
        let mut terms = Vec::with_capacity(row.terms.len());
        for term in row.terms.iter().copied() {
            if let TypeKind::Param(param) = types.kind(term)
                && param.decl_file.as_os_str() == EFFECT_ROW_PARAM_DECL_FILE
            {
                let Some(param_key) = effect_params.resolve_effect_param(param) else {
                    return Err(CanonicalEncodingError::MissingEffectParamKey {
                        param_name: param.name.clone(),
                    });
                };
                terms.push(EffectTerm::param(
                    param_key.owner().clone(),
                    param_key.ordinal(),
                    param_key.name().to_string(),
                ));
                continue;
            }
            terms.push(EffectTerm::Concrete {
                type_key: CanonicalTextKey::new(encoder.encode_type(term, 0)?),
            });
        }
        Ok(Self::new(terms, closed))
    }

    pub fn from_concrete_effect_row<R>(
        types: &TypeStore,
        row: &EffectRow,
        type_params: &R,
        closed: bool,
    ) -> Result<Self, CanonicalEncodingError>
    where
        R: StableTypeParamResolver + ?Sized,
    {
        Self::from_effect_row(types, row, type_params, &NoEffectParamResolver, closed)
    }

    pub fn to_effect_row_with<R>(&self, mut resolve: R) -> Result<EffectRow, CanonicalEncodingError>
    where
        R: FnMut(&CanonicalTextKey) -> Option<TypeId>,
    {
        let mut terms = Vec::with_capacity(self.terms.len());
        for term in &self.terms {
            match term {
                EffectTerm::Concrete { type_key } => {
                    let Some(ty) = resolve(type_key) else {
                        return Err(CanonicalEncodingError::UnresolvedEffectTypeKey {
                            type_key: type_key.as_str().to_string(),
                        });
                    };
                    terms.push(ty);
                }
                EffectTerm::Param { name, .. } => {
                    return Err(CanonicalEncodingError::UnsubstitutedEffectParam {
                        param_name: name.clone(),
                    });
                }
            }
        }
        Ok(EffectRow::new(terms))
    }

    pub fn substitute(&self, bindings: &HashMap<StableEffectParamKey, EffectRowTemplate>) -> Self {
        let mut terms = Vec::new();
        let mut substituted = false;
        let mut retained_source_term = false;
        let mut substituted_rows_closed = true;
        for term in &self.terms {
            if let Some(param_key) = term.param_key()
                && let Some(binding) = bindings.get(&param_key)
            {
                substituted = true;
                substituted_rows_closed &= binding.closed;
                terms.extend(binding.terms.iter().cloned());
                continue;
            }
            retained_source_term = true;
            terms.push(term.clone());
        }
        let closed =
            self.closed || (substituted && !retained_source_term && substituted_rows_closed);
        Self::new(terms, closed)
    }

    pub fn terms(&self) -> &[EffectTerm] {
        &self.terms
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    pub fn is_pure(&self) -> bool {
        self.terms.is_empty()
    }

    pub fn is_pure_closed(&self) -> bool {
        self.closed && self.terms.is_empty()
    }
}

impl StableCanonicalKey for EffectRowTemplate {
    fn canonical_text(&self) -> String {
        let mut text = format!(
            "E({})",
            self.terms
                .iter()
                .map(StableCanonicalKey::canonical_text)
                .collect::<Vec<_>>()
                .join(",")
        );
        if self.closed {
            text.push('!');
        }
        text
    }
}

impl fmt::Display for EffectRowTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical_text())
    }
}

/// Semantic monomorphic instance identity derived from canonical type/effect text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StableInstanceKey {
    template: StableTemplateKey,
    canonical_type_args: Vec<String>,
    effect_arg_templates: Vec<EffectRowTemplate>,
}

impl StableInstanceKey {
    pub fn from_canonical_args(
        template: StableTemplateKey,
        canonical_type_args: Vec<String>,
        canonical_effect_args: Vec<String>,
    ) -> Self {
        let effect_arg_templates = canonical_effect_args
            .into_iter()
            .map(EffectRowTemplate::from_canonical_text)
            .collect::<Result<Vec<_>, _>>()
            .expect("canonical effect arguments must be effect-row template text");
        Self::from_effect_arg_templates(template, canonical_type_args, effect_arg_templates)
    }

    pub fn from_effect_arg_templates(
        template: StableTemplateKey,
        canonical_type_args: Vec<String>,
        effect_arg_templates: Vec<EffectRowTemplate>,
    ) -> Self {
        Self {
            template,
            canonical_type_args,
            effect_arg_templates,
        }
    }

    pub fn from_type_arguments<R>(
        template: StableTemplateKey,
        types: &TypeStore,
        type_args: &[TypeId],
        effect_args: &[EffectRow],
        type_params: &R,
    ) -> Result<Self, CanonicalEncodingError>
    where
        R: StableTypeParamResolver + ?Sized,
    {
        let canonical_type_args = type_args
            .iter()
            .copied()
            .map(|ty| canonical_type_text(types, ty, type_params))
            .collect::<Result<Vec<_>, _>>()?;
        let canonical_effect_args = effect_args
            .iter()
            .map(|row| EffectRowTemplate::from_concrete_effect_row(types, row, type_params, false))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self::from_effect_arg_templates(
            template,
            canonical_type_args,
            canonical_effect_args,
        ))
    }

    pub fn template(&self) -> &StableTemplateKey {
        &self.template
    }

    pub fn canonical_type_args(&self) -> &[String] {
        &self.canonical_type_args
    }

    pub fn effect_arg_templates(&self) -> &[EffectRowTemplate] {
        &self.effect_arg_templates
    }

    pub fn canonical_effect_args(&self) -> Vec<String> {
        self.effect_arg_templates
            .iter()
            .map(StableCanonicalKey::canonical_text)
            .collect()
    }
}

impl StableCanonicalKey for StableInstanceKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "instance",
            [
                self.template.canonical_text(),
                canonical_list(&self.canonical_type_args),
                canonical_list(&self.canonical_effect_args()),
            ],
        )
    }
}

impl StableSymbolKey for StableInstanceKey {
    fn readable_path(&self) -> &str {
        self.template.readable_path()
    }
}

/// Semantic identity for a dispatch target selected by upstream resolution.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DispatchTargetKey {
    dispatch_kind: String,
    stable_instance_key: StableInstanceKey,
}

impl DispatchTargetKey {
    pub fn new(dispatch_kind: impl Into<String>, stable_instance_key: StableInstanceKey) -> Self {
        Self {
            dispatch_kind: dispatch_kind.into(),
            stable_instance_key,
        }
    }

    pub fn dispatch_kind(&self) -> &str {
        &self.dispatch_kind
    }

    pub fn stable_instance_key(&self) -> &StableInstanceKey {
        &self.stable_instance_key
    }
}

impl StableCanonicalKey for DispatchTargetKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "dispatch_target",
            [
                self.dispatch_kind.clone(),
                self.stable_instance_key.canonical_text(),
            ],
        )
    }
}

impl StableSymbolKey for DispatchTargetKey {
    fn readable_path(&self) -> &str {
        self.stable_instance_key.readable_path()
    }
}

/// Semantic identity for a call target; display names remain diagnostic only.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CallTargetKey {
    Direct {
        stable_instance_key: StableInstanceKey,
    },
    Dispatch {
        dispatch_target_key: DispatchTargetKey,
    },
}

impl CallTargetKey {
    pub fn direct(stable_instance_key: StableInstanceKey) -> Self {
        Self::Direct {
            stable_instance_key,
        }
    }

    pub fn dispatch(dispatch_target_key: DispatchTargetKey) -> Self {
        Self::Dispatch {
            dispatch_target_key,
        }
    }
}

impl StableCanonicalKey for CallTargetKey {
    fn canonical_text(&self) -> String {
        match self {
            Self::Direct {
                stable_instance_key,
            } => canonical_record(
                "call_target",
                ["direct".to_string(), stable_instance_key.canonical_text()],
            ),
            Self::Dispatch {
                dispatch_target_key,
            } => canonical_record(
                "call_target",
                ["dispatch".to_string(), dispatch_target_key.canonical_text()],
            ),
        }
    }
}

impl StableSymbolKey for CallTargetKey {
    fn readable_path(&self) -> &str {
        match self {
            Self::Direct {
                stable_instance_key,
            } => stable_instance_key.readable_path(),
            Self::Dispatch {
                dispatch_target_key,
            } => dispatch_target_key.readable_path(),
        }
    }
}

/// ABI symbol identity composed from the ABI namespace and semantic target key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct AbiSymbolKey {
    kind: AbiSymbolKind,
    target_canonical_text: String,
    readable_path: String,
}

impl AbiSymbolKey {
    pub fn new<K>(kind: AbiSymbolKind, target: &K) -> Self
    where
        K: StableSymbolKey + ?Sized,
    {
        Self {
            kind,
            target_canonical_text: target.canonical_text(),
            readable_path: target.readable_path().to_string(),
        }
    }

    pub fn kind(&self) -> AbiSymbolKind {
        self.kind
    }
}

impl StableCanonicalKey for AbiSymbolKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "abi_symbol",
            [
                abi_symbol_kind_text(self.kind).to_string(),
                self.target_canonical_text.clone(),
            ],
        )
    }
}

impl StableSymbolKey for AbiSymbolKey {
    fn readable_path(&self) -> &str {
        &self.readable_path
    }
}

fn abi_symbol_kind_text(kind: AbiSymbolKind) -> &'static str {
    match kind {
        AbiSymbolKind::Fun => "fun",
        AbiSymbolKind::Global => "global",
        AbiSymbolKind::Type => "type",
    }
}

/// Closure identity anchored by owner semantic key and lexical path.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StableClosureKey {
    owner_canonical_text: String,
    readable_path: String,
    lexical_path: String,
}

impl StableClosureKey {
    pub fn new(owner: &impl StableSymbolKey, lexical_path: impl Into<String>) -> Self {
        let lexical_path = lexical_path.into();
        let readable_path = if owner.readable_path().is_empty() {
            lexical_path.clone()
        } else if lexical_path.is_empty() {
            owner.readable_path().to_string()
        } else {
            format!("{}.{}", owner.readable_path(), lexical_path)
        };
        Self {
            owner_canonical_text: owner.canonical_text(),
            readable_path,
            lexical_path,
        }
    }

    pub fn lexical_path(&self) -> &str {
        &self.lexical_path
    }

    pub fn env_canonical_name(&self) -> String {
        canonical_record("closure_env", [self.canonical_text()])
    }
}

impl StableCanonicalKey for StableClosureKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "closure",
            [self.owner_canonical_text.clone(), self.lexical_path.clone()],
        )
    }
}

impl StableSymbolKey for StableClosureKey {
    fn readable_path(&self) -> &str {
        &self.readable_path
    }
}

/// Stable effect step schema identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StableEffectSchemaKey {
    owner_canonical_text: String,
    schema_role: String,
    semantic_fragments: Vec<String>,
}

impl StableEffectSchemaKey {
    pub fn new(
        owner: &impl StableCanonicalKey,
        schema_role: impl Into<String>,
        semantic_fragments: Vec<String>,
    ) -> Self {
        Self {
            owner_canonical_text: owner.canonical_text(),
            schema_role: schema_role.into(),
            semantic_fragments,
        }
    }
}

impl StableCanonicalKey for StableEffectSchemaKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "effect_schema",
            [
                self.owner_canonical_text.clone(),
                self.schema_role.clone(),
                canonical_list(&self.semantic_fragments),
            ],
        )
    }
}

/// Stable continuation schema identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StableContinuationSchemaKey {
    owner_canonical_text: String,
    schema_role: String,
    semantic_fragments: Vec<String>,
}

impl StableContinuationSchemaKey {
    pub fn new(
        owner: &impl StableCanonicalKey,
        schema_role: impl Into<String>,
        semantic_fragments: Vec<String>,
    ) -> Self {
        Self {
            owner_canonical_text: owner.canonical_text(),
            schema_role: schema_role.into(),
            semantic_fragments,
        }
    }
}

impl StableCanonicalKey for StableContinuationSchemaKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "continuation_schema",
            [
                self.owner_canonical_text.clone(),
                self.schema_role.clone(),
                canonical_list(&self.semantic_fragments),
            ],
        )
    }
}

/// Stable private boundary identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StableBoundaryKey {
    owner_canonical_text: String,
    structural_role: String,
    source_anchor: Option<String>,
}

impl StableBoundaryKey {
    pub fn new(
        owner: &impl StableCanonicalKey,
        structural_role: impl Into<String>,
        source_anchor: Option<String>,
    ) -> Self {
        Self {
            owner_canonical_text: owner.canonical_text(),
            structural_role: structural_role.into(),
            source_anchor,
        }
    }
}

impl StableCanonicalKey for StableBoundaryKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "boundary",
            [
                self.owner_canonical_text.clone(),
                self.structural_role.clone(),
                self.source_anchor.clone().unwrap_or_default(),
            ],
        )
    }
}

/// Stable state identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StableStateKey {
    owner_canonical_text: String,
    structural_role: String,
    source_anchor: Option<String>,
}

impl StableStateKey {
    pub fn new(
        owner: &impl StableCanonicalKey,
        structural_role: impl Into<String>,
        source_anchor: Option<String>,
    ) -> Self {
        Self {
            owner_canonical_text: owner.canonical_text(),
            structural_role: structural_role.into(),
            source_anchor,
        }
    }
}

impl StableCanonicalKey for StableStateKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "state",
            [
                self.owner_canonical_text.clone(),
                self.structural_role.clone(),
                self.source_anchor.clone().unwrap_or_default(),
            ],
        )
    }
}

/// Stable frame-slot identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StableFrameSlotKey {
    owner_canonical_text: String,
    structural_role: String,
    source_anchor: Option<String>,
}

impl StableFrameSlotKey {
    pub fn new(
        owner: &impl StableCanonicalKey,
        structural_role: impl Into<String>,
        source_anchor: Option<String>,
    ) -> Self {
        Self {
            owner_canonical_text: owner.canonical_text(),
            structural_role: structural_role.into(),
            source_anchor,
        }
    }
}

impl StableCanonicalKey for StableFrameSlotKey {
    fn canonical_text(&self) -> String {
        canonical_record(
            "frame_slot",
            [
                self.owner_canonical_text.clone(),
                self.structural_role.clone(),
                self.source_anchor.clone().unwrap_or_default(),
            ],
        )
    }
}

/// Stable type-parameter identity used by canonical type text.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct StableTypeParamKey {
    owner_def_key: String,
    index: usize,
}

impl StableTypeParamKey {
    pub fn new(owner_def_key: impl Into<String>, index: usize) -> Self {
        Self {
            owner_def_key: owner_def_key.into(),
            index,
        }
    }

    pub fn owner_def_key(&self) -> &str {
        &self.owner_def_key
    }

    pub fn index(&self) -> usize {
        self.index
    }

    fn canonical_text(&self) -> String {
        format!("{}#{}", self.owner_def_key, self.index)
    }
}

/// Resolves type parameters to a stable owner/index key.
pub trait StableTypeParamResolver {
    fn resolve(&self, param: &TypeParamType) -> Option<StableTypeParamKey>;
}

impl<F> StableTypeParamResolver for F
where
    F: Fn(&TypeParamType) -> Option<StableTypeParamKey>,
{
    fn resolve(&self, param: &TypeParamType) -> Option<StableTypeParamKey> {
        self(param)
    }
}

impl StableTypeParamResolver for HashMap<TypeParamType, StableTypeParamKey> {
    fn resolve(&self, param: &TypeParamType) -> Option<StableTypeParamKey> {
        self.get(param).cloned()
    }
}

/// Resolver for canonical text that is guaranteed to fail on type parameters.
#[derive(Debug, Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct NoTypeParamResolver;

impl StableTypeParamResolver for NoTypeParamResolver {
    fn resolve(&self, _: &TypeParamType) -> Option<StableTypeParamKey> {
        None
    }
}

/// Errors raised while building canonical type/effect text.
#[derive(Debug, Clone, PartialEq, Eq, Error, serde::Serialize, serde::Deserialize)]
pub enum CanonicalEncodingError {
    #[error("missing stable type parameter key for `{param_name}`")]
    MissingTypeParamKey { param_name: String },
    #[error("missing stable effect parameter key for `{param_name}`")]
    MissingEffectParamKey { param_name: String },
    #[error("invalid effect row template canonical text `{text}`")]
    InvalidEffectRowTemplateText { text: String },
    #[error("unresolved effect type key `{type_key}`")]
    UnresolvedEffectTypeKey { type_key: String },
    #[error("effect row template still contains unsubstituted effect parameter `{param_name}`")]
    UnsubstitutedEffectParam { param_name: String },
    #[error("canonical type encoding exceeded recursion limit")]
    RecursionLimit,
}

/// Encodes a type into the canonical text used by stable-id hashes.
pub fn canonical_type_text<R>(
    types: &TypeStore,
    ty: TypeId,
    type_params: &R,
) -> Result<String, CanonicalEncodingError>
where
    R: StableTypeParamResolver + ?Sized,
{
    let mut encoder = CanonicalEncoder::new(types, type_params);
    encoder.encode_type(ty, 0)
}

/// Encodes an effect row into the canonical text used by stable-id hashes.
pub fn canonical_effect_row_text<R>(
    types: &TypeStore,
    row: &EffectRow,
    type_params: &R,
) -> Result<String, CanonicalEncodingError>
where
    R: StableTypeParamResolver + ?Sized,
{
    let mut encoder = CanonicalEncoder::new(types, type_params);
    encoder.encode_effect_row(row, false, 0)
}

/// Encodes a callable overload signature from canonical function type text plus generic arity.
pub fn canonical_callable_signature_key<R>(
    types: &TypeStore,
    callable_ty: TypeId,
    owner_type_param_count: usize,
    callable_type_param_count: usize,
    effect_param_count: usize,
    type_params: &R,
) -> Result<String, CanonicalEncodingError>
where
    R: StableTypeParamResolver + ?Sized,
{
    Ok(canonical_record(
        "callable_sig",
        [
            owner_type_param_count.to_string(),
            callable_type_param_count.to_string(),
            effect_param_count.to_string(),
            canonical_type_text(types, callable_ty, type_params)?,
        ],
    ))
}

/// Encodes a property getter overload signature from canonical return type text plus owner arity.
pub fn canonical_property_getter_signature_key<R>(
    types: &TypeStore,
    return_ty: TypeId,
    owner_type_param_count: usize,
    type_params: &R,
) -> Result<String, CanonicalEncodingError>
where
    R: StableTypeParamResolver + ?Sized,
{
    Ok(canonical_record(
        "getter_sig",
        [
            owner_type_param_count.to_string(),
            canonical_type_text(types, return_ty, type_params)?,
        ],
    ))
}

/// Canonical RTTI identity key for a semantic type.
pub fn stable_rtti_type_key_for_type<R>(
    types: &TypeStore,
    ty: TypeId,
    type_params: &R,
) -> Result<CanonicalTextKey, CanonicalEncodingError>
where
    R: StableTypeParamResolver + ?Sized,
{
    canonical_type_text(types, ty, type_params).map(CanonicalTextKey::new)
}

/// Stable RTTI type-id derived from semantic canonical type text.
pub fn stable_rtti_type_id_for_type<R>(
    types: &TypeStore,
    ty: TypeId,
    type_params: &R,
) -> Result<u64, CanonicalEncodingError>
where
    R: StableTypeParamResolver + ?Sized,
{
    let key = stable_rtti_type_key_for_type(types, ty, type_params)?;
    Ok(stable_rtti_type_id(key.as_str()))
}

/// Derived RTTI/type-descriptor keys keep wrapper roles distinct from the wrapped type itself.
pub fn stable_rtti_derived_type_key(role: &str, base_type_key: &str) -> CanonicalTextKey {
    CanonicalTextKey::new(canonical_record(
        "rtti_desc",
        [role.to_string(), base_type_key.to_string()],
    ))
}

/// Canonical RTTI identity key for a bare nominal type with no type/effect arguments.
pub fn canonical_nominal_type_key(fqn: &str) -> CanonicalTextKey {
    CanonicalTextKey::new(canonical_nominal_text(fqn, None, None))
}

/// Shared RTTI interface-id helper for interface declaration identities.
pub fn stable_rtti_interface_id(canonical_name: &str) -> u64 {
    stable_hash64(StableHashScope::RttiV0, canonical_name)
}

/// Overload suffixes stay short, but now hash the shared stable template key.
pub fn stable_template_symbol_suffix(template: &StableTemplateKey) -> String {
    format!(
        "{:016x}",
        stable_hash64(StableHashScope::AbiV0, &template.canonical_text())
    )
}

struct CanonicalEncoder<'a, R: ?Sized> {
    types: &'a TypeStore,
    type_params: &'a R,
    cache: HashMap<TypeId, String>,
}

impl<'a, R> CanonicalEncoder<'a, R>
where
    R: StableTypeParamResolver + ?Sized,
{
    fn new(types: &'a TypeStore, type_params: &'a R) -> Self {
        Self {
            types,
            type_params,
            cache: HashMap::new(),
        }
    }

    fn encode_type(&mut self, ty: TypeId, depth: usize) -> Result<String, CanonicalEncodingError> {
        if let Some(cached) = self.cache.get(&ty) {
            return Ok(cached.clone());
        }
        if depth > MAX_CANONICAL_DEPTH {
            return Err(CanonicalEncodingError::RecursionLimit);
        }

        let encoded = match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::Any) => "R(Any)".to_string(),
            TypeKind::Ref(RefTypeKind::String) => "R(String)".to_string(),
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                self.encode_nominal(nominal, depth + 1)?
            }
            TypeKind::Ref(RefTypeKind::Function(fun)) => {
                let receiver = match fun.receiver {
                    Some(receiver) => self.encode_type(receiver, depth + 1)?,
                    None => "-".to_string(),
                };
                let params = self.encode_type_list(&fun.params, depth + 1)?;
                let return_ty = self.encode_type(fun.return_ty, depth + 1)?;
                let row = self.encode_effect_row(&fun.effects, fun.effects_closed, depth + 1)?;
                format!("F({receiver};[{params}]->{return_ty}/{row})")
            }
            TypeKind::Ref(RefTypeKind::Union(union)) => self.encode_union(union, depth + 1)?,
            TypeKind::Value(ValueTypeKind::Unit) => "V(Unit)".to_string(),
            TypeKind::Value(ValueTypeKind::Nothing) => "V(Nothing)".to_string(),
            TypeKind::Value(ValueTypeKind::Bool) => "V(Bool)".to_string(),
            TypeKind::Value(ValueTypeKind::Char) => "V(Char)".to_string(),
            TypeKind::Value(ValueTypeKind::Float64) => "V(Float64)".to_string(),
            TypeKind::Value(ValueTypeKind::Float32) => "V(Float32)".to_string(),
            TypeKind::Value(ValueTypeKind::Int) => "V(Int)".to_string(),
            TypeKind::Value(ValueTypeKind::UInt) => "V(UInt)".to_string(),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => format!("V(Int{bits})"),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => format!("V(UInt{bits})"),
            TypeKind::Value(ValueTypeKind::Option(inner)) => {
                let inner = self.encode_type(*inner, depth + 1)?;
                format!("V(Option<{inner}>)")
            }
            TypeKind::Value(ValueTypeKind::Tuple(elements)) => {
                let elements = self.encode_type_list(elements, depth + 1)?;
                format!("T({elements})")
            }
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                self.encode_nominal(nominal, depth + 1)?
            }
            TypeKind::StarProjection(star) => {
                let read_ty = self.encode_type(star.read_ty, depth + 1)?;
                format!("S({read_ty})")
            }
            TypeKind::Param(param) => {
                let Some(param_key) = self.type_params.resolve(param) else {
                    return Err(CanonicalEncodingError::MissingTypeParamKey {
                        param_name: param.name.clone(),
                    });
                };
                format!("P({})", param_key.canonical_text())
            }
        };

        self.cache.insert(ty, encoded.clone());
        Ok(encoded)
    }

    fn encode_nominal(
        &mut self,
        nominal: &NominalType,
        depth: usize,
    ) -> Result<String, CanonicalEncodingError> {
        let args = if nominal.args.is_empty() {
            None
        } else {
            Some(self.encode_type_list(&nominal.args, depth + 1)?)
        };
        let eff = match &nominal.eff {
            Some(eff) => Some(self.encode_effect_row(eff, false, depth + 1)?),
            None => None,
        };
        Ok(canonical_nominal_text(
            &nominal.fqn,
            args.as_deref(),
            eff.as_deref(),
        ))
    }

    fn encode_union(
        &mut self,
        union: &UnionType,
        depth: usize,
    ) -> Result<String, CanonicalEncodingError> {
        let mut variants = union
            .variants
            .iter()
            .copied()
            .map(|variant| self.encode_type(variant, depth + 1))
            .collect::<Result<Vec<_>, _>>()?;
        variants.sort();
        variants.dedup();
        Ok(format!("U({})", variants.join(",")))
    }

    fn encode_effect_row(
        &mut self,
        row: &EffectRow,
        closed: bool,
        depth: usize,
    ) -> Result<String, CanonicalEncodingError> {
        let terms = row
            .terms
            .iter()
            .copied()
            .map(|term| {
                self.encode_type(term, depth + 1)
                    .map(|type_key| EffectTerm::Concrete {
                        type_key: CanonicalTextKey::new(type_key),
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(EffectRowTemplate::new(terms, closed).canonical_text())
    }

    fn encode_type_list(
        &mut self,
        types: &[TypeId],
        depth: usize,
    ) -> Result<String, CanonicalEncodingError> {
        types
            .iter()
            .copied()
            .map(|ty| self.encode_type(ty, depth + 1))
            .collect::<Result<Vec<_>, _>>()
            .map(|parts| parts.join(","))
    }
}

fn canonical_nominal_text(fqn: &str, args: Option<&str>, eff: Option<&str>) -> String {
    let mut encoded = format!("N({fqn}");
    if args.is_some() || eff.is_some() {
        encoded.push('<');
        if let Some(args) = args
            && !args.is_empty()
        {
            encoded.push_str(args);
        }
        if let Some(eff) = eff {
            if args.is_some_and(|args| !args.is_empty()) {
                encoded.push(';');
            }
            encoded.push_str("eff=");
            encoded.push_str(eff);
        }
        encoded.push('>');
    }
    encoded.push(')');
    encoded
}

fn split_top_level_effect_terms(body: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut paren_depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut angle_depth = 0i32;
    for (idx, ch) in body.char_indices() {
        match ch {
            ',' if paren_depth == 0 && bracket_depth == 0 && angle_depth == 0 => {
                parts.push(body[start..idx].to_string());
                start = idx + ch.len_utf8();
            }
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            '<' => angle_depth += 1,
            '>' => angle_depth -= 1,
            _ => {}
        }
    }
    parts.push(body[start..].to_string());
    parts
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use super::*;
    use crate::cone::{ConeKind, ConeManifest, ConeNativeBuildConfig, ConeSection};
    use scoopc_span::Span;

    fn test_manifest(name: &str, version: &str) -> ConeManifest {
        ConeManifest {
            cone: ConeSection {
                name: name.to_string(),
                version: version.to_string(),
                kind: ConeKind::Bin,
            },
            dependencies: Default::default(),
            pre_specialize_functions: Vec::new(),
            pre_specialize_types: Vec::new(),
            export_entry_points: Vec::new(),
            selectors: Vec::new(),
            native_build: ConeNativeBuildConfig::default(),
        }
    }

    #[test]
    fn canonical_type_text_encodes_required_shapes() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        let param = TypeParamType {
            name: "T".to_string(),
            decl_file: PathBuf::from("fixtures/main.scoop"),
            decl_span: Span::new(3, 4),
        };
        let param_ty = types.ty_param(param.clone());
        let async_eff = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.Async".to_string(),
            args: Vec::new(),
            eff: None,
        })));
        let service = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.Service".to_string(),
            args: vec![param_ty],
            eff: Some(EffectRow::new(vec![async_eff])),
        })));
        let left = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.Left".to_string(),
            args: vec![param_ty],
            eff: None,
        })));
        let right = types.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.Right".to_string(),
            args: vec![builtins.string],
            eff: None,
        })));
        let union = types.ty_union(vec![right, left]);
        let tuple = types.ty_tuple(vec![builtins.int, param_ty, service]);
        let fun = types.ty_function(
            Some(builtins.string),
            vec![tuple],
            union,
            EffectRow::new(vec![async_eff]),
            true,
        );
        let resolver = HashMap::from([(param, StableTypeParamKey::new("pkg.main", 0))]);

        assert_eq!(
            canonical_type_text(&types, service, &resolver).unwrap(),
            "N(pkg.Service<P(pkg.main#0);eff=E(N(pkg.Async))>)"
        );
        assert_eq!(
            canonical_type_text(&types, fun, &resolver).unwrap(),
            "F(R(String);[T(V(Int),P(pkg.main#0),N(pkg.Service<P(pkg.main#0);eff=E(N(pkg.Async))>))]->U(N(pkg.Left<P(pkg.main#0)>),N(pkg.Right<R(String)>))/E(N(pkg.Async))!)"
        );
    }

    #[test]
    fn canonical_effect_row_text_is_stable_across_term_order_and_intern_order() {
        let mut types_a = TypeStore::new();
        let eff_a_a = types_a.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.EffectA".to_string(),
            args: Vec::new(),
            eff: None,
        })));
        let eff_b_a = types_a.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.EffectB".to_string(),
            args: Vec::new(),
            eff: None,
        })));

        let mut types_b = TypeStore::new();
        let eff_b_b = types_b.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.EffectB".to_string(),
            args: Vec::new(),
            eff: None,
        })));
        let eff_a_b = types_b.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.EffectA".to_string(),
            args: Vec::new(),
            eff: None,
        })));

        let row_a = EffectRow::new(vec![eff_b_a, eff_a_a, eff_b_a]);
        let row_b = EffectRow::new(vec![eff_a_b, eff_b_b]);

        assert_eq!(
            canonical_effect_row_text(&types_a, &row_a, &NoTypeParamResolver).unwrap(),
            canonical_effect_row_text(&types_b, &row_b, &NoTypeParamResolver).unwrap()
        );
        assert_eq!(
            canonical_effect_row_text(&types_a, &row_a, &NoTypeParamResolver).unwrap(),
            "E(N(pkg.EffectA),N(pkg.EffectB))"
        );
    }

    #[test]
    fn effect_row_template_is_stable_and_aligns_concrete_type_keys() {
        let mut types_a = TypeStore::new();
        let eff_b_a = types_a.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.EffectB".to_string(),
            args: Vec::new(),
            eff: None,
        })));
        let eff_a_a = types_a.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.EffectA".to_string(),
            args: Vec::new(),
            eff: None,
        })));

        let mut types_b = TypeStore::new();
        let eff_a_b = types_b.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.EffectA".to_string(),
            args: Vec::new(),
            eff: None,
        })));
        let eff_b_b = types_b.intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
            fqn: "pkg.EffectB".to_string(),
            args: Vec::new(),
            eff: None,
        })));

        let template_a = EffectRowTemplate::from_concrete_effect_row(
            &types_a,
            &EffectRow::new(vec![eff_b_a, eff_a_a, eff_b_a]),
            &NoTypeParamResolver,
            false,
        )
        .unwrap();
        let template_b = EffectRowTemplate::from_concrete_effect_row(
            &types_b,
            &EffectRow::new(vec![eff_a_b, eff_b_b]),
            &NoTypeParamResolver,
            false,
        )
        .unwrap();
        let effect_a_key =
            stable_rtti_type_key_for_type(&types_a, eff_a_a, &NoTypeParamResolver).unwrap();

        assert_eq!(template_a.canonical_text(), template_b.canonical_text());
        assert_eq!(
            template_a.canonical_text(),
            "E(N(pkg.EffectA),N(pkg.EffectB))"
        );
        assert!(matches!(
            &template_a.terms()[0],
            EffectTerm::Concrete { type_key } if type_key.as_str() == effect_a_key.as_str()
        ));

        let local_types = HashMap::from([
            ("N(pkg.EffectA)".to_string(), eff_a_a),
            ("N(pkg.EffectB)".to_string(), eff_b_a),
        ]);
        let round_tripped = template_a
            .to_effect_row_with(|key| local_types.get(key.as_str()).copied())
            .unwrap();
        assert_eq!(round_tripped, EffectRow::new(vec![eff_a_a, eff_b_a]));
    }

    #[test]
    fn effect_row_template_substitution_flattens_params_and_preserves_pure_closed() {
        let owner = StableDefKey::new(
            StableConeKey::new("demo", "0.1.0"),
            StableDefNamespace::Fun,
            "pkg.run",
            "generic_fun",
            None,
        );
        let param_key = StableEffectParamKey::new(owner.clone(), 0, "E");
        let param_row =
            EffectRowTemplate::new(vec![EffectTerm::param(owner.clone(), 0, "E")], false);
        let pure_closed = param_row.substitute(&HashMap::from([(
            param_key.clone(),
            EffectRowTemplate::pure_closed(),
        )]));

        assert!(pure_closed.is_pure_closed());
        assert_eq!(pure_closed.canonical_text(), "E()!");

        let mixed_row = EffectRowTemplate::new(
            vec![
                EffectTerm::param(owner, 0, "E"),
                EffectTerm::concrete("N(pkg.IO)"),
            ],
            false,
        );
        let substituted = mixed_row.substitute(&HashMap::from([(
            param_key,
            EffectRowTemplate::new(vec![EffectTerm::concrete("N(pkg.Async)")], false),
        )]));

        assert_eq!(substituted.canonical_text(), "E(N(pkg.Async),N(pkg.IO))");
        assert!(!substituted.closed());
    }

    #[test]
    fn effect_row_template_tracks_closed_pure_separately_from_open_pure() {
        let open = EffectRowTemplate::pure();
        let closed = EffectRowTemplate::pure_closed();
        let closed_non_pure = EffectRowTemplate::new(vec![EffectTerm::concrete("N(pkg.IO)")], true);

        assert!(open.is_pure());
        assert!(!open.is_pure_closed());
        assert!(closed.is_pure_closed());
        assert_eq!(closed.canonical_text(), "E()!");
        assert!(closed_non_pure.closed());
        assert!(!closed_non_pure.is_pure_closed());
        assert_eq!(closed_non_pure.canonical_text(), "E(N(pkg.IO))!");
    }

    #[test]
    fn stable_instance_key_keeps_structured_effect_arg_templates() {
        let template = StableTemplateKey::new(StableDefKey::new(
            StableConeKey::new("demo", "0.1.0"),
            StableDefNamespace::Fun,
            "pkg.effectful",
            "generic_fun",
            None,
        ));
        let effect_template =
            EffectRowTemplate::new(vec![EffectTerm::concrete("N(pkg.Async)")], true);
        let instance = StableInstanceKey::from_effect_arg_templates(
            template,
            vec!["V(Int)".to_string()],
            vec![effect_template.clone()],
        );

        assert_eq!(instance.effect_arg_templates(), &[effect_template]);
        assert_eq!(
            instance.canonical_effect_args(),
            vec!["E(N(pkg.Async))!".to_string()]
        );
        assert!(instance.canonical_text().contains("E(N(pkg.Async))!"));
    }

    #[test]
    fn call_dispatch_and_abi_keys_separate_semantic_identity_from_display() {
        let template = StableTemplateKey::new(StableDefKey::new(
            StableConeKey::new("demo", "0.1.0"),
            StableDefNamespace::Fun,
            "pkg.Service.run",
            "generic_fun",
            None,
        ));
        let instance = StableInstanceKey::from_effect_arg_templates(
            template,
            Vec::new(),
            vec![EffectRowTemplate::pure()],
        );
        let dispatch_target = DispatchTargetKey::new("interface_slot", instance.clone());
        let direct_call = CallTargetKey::direct(instance.clone());
        let dispatch_call = CallTargetKey::dispatch(dispatch_target.clone());
        let abi_fun = AbiSymbolKey::new(AbiSymbolKind::Fun, &instance);
        let abi_global = AbiSymbolKey::new(AbiSymbolKind::Global, &instance);

        assert_ne!(direct_call.canonical_text(), dispatch_call.canonical_text());
        assert!(dispatch_target.canonical_text().contains("interface_slot"));
        assert_eq!(direct_call.readable_path(), "pkg.Service.run");
        assert_ne!(abi_fun.canonical_text(), abi_global.canonical_text());
        assert_eq!(abi_fun.readable_path(), "pkg.Service.run");
    }

    #[test]
    fn stable_id_hash_prefixes_do_not_collide() {
        let canonical = "N(pkg.Token)";

        assert_ne!(
            stable_hash128_hex(StableHashScope::AbiV0, canonical),
            stable_hash128_hex(StableHashScope::PrivateV0, canonical)
        );
        assert_ne!(
            stable_hash128_hex(StableHashScope::AbiV0, canonical),
            stable_hash128_hex(StableHashScope::RttiV0, canonical)
        );
        assert_ne!(
            stable_hash64(StableHashScope::AbiV0, canonical),
            stable_hash64(StableHashScope::DumpV0, canonical)
        );
    }

    #[test]
    fn stable_rtti_derived_type_keys_keep_wrapper_roles_distinct() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let base = stable_rtti_type_key_for_type(&types, builtins.int, &NoTypeParamResolver)
            .expect("builtin type should have a canonical RTTI key");
        let value_box = stable_rtti_derived_type_key("mir_value_box_type_desc", base.as_str());

        assert_eq!(base.as_str(), "V(Int)");
        assert_ne!(
            stable_rtti_type_id(base.as_str()),
            stable_rtti_type_id(value_box.as_str())
        );
        assert!(value_box.as_str().contains("mir_value_box_type_desc"));
    }

    #[test]
    fn canonical_type_text_uses_explicit_type_param_keys_instead_of_pretty_text() {
        let mut types = TypeStore::new();
        let param = TypeParamType {
            name: "PrettyName".to_string(),
            decl_file: PathBuf::from("/tmp/not-for-canonical-input.scoop"),
            decl_span: Span::new(11, 42),
        };
        let ty = types.ty_param(param.clone());
        let resolver = HashMap::from([(param, StableTypeParamKey::new("pkg.owner.fun", 2))]);

        let encoded = canonical_type_text(&types, ty, &resolver).unwrap();
        assert_eq!(encoded, "P(pkg.owner.fun#2)");
        assert!(!encoded.contains("PrettyName"));
        assert!(!encoded.contains("not-for-canonical-input"));
    }

    #[test]
    fn stable_id_dump_label_uses_dump_scope() {
        let canonical = "site(pkg.owner.fun#2)";
        let label = stable_dump_label("site", canonical);

        assert_eq!(
            label,
            format!(
                "site#h{}",
                stable_hash128_hex(StableHashScope::DumpV0, canonical)
            )
        );
        assert_ne!(
            label,
            format!(
                "site#h{}",
                stable_hash128_hex(StableHashScope::AbiV0, canonical)
            )
        );
    }

    #[test]
    fn stable_id_missing_type_param_key_is_an_error() {
        let mut types = TypeStore::new();
        let ty = types.ty_param(TypeParamType {
            name: "T".to_string(),
            decl_file: PathBuf::from("fixtures/main.scoop"),
            decl_span: Span::new(1, 2),
        });

        assert_eq!(
            canonical_type_text(&types, ty, &NoTypeParamResolver),
            Err(CanonicalEncodingError::MissingTypeParamKey {
                param_name: "T".to_string(),
            })
        );
    }

    #[test]
    fn stable_cone_key_reads_manifest_and_virtual_source_path() {
        let manifest = test_manifest("demo-cone", "1.2.3");
        let explicit = StableConeKey::from_manifest(&manifest);
        let virtual_key = StableConeKey::for_virtual_source_path(Path::new("/tmp/example.scoop"));

        assert_eq!(explicit.name(), "demo-cone");
        assert_eq!(explicit.version(), "1.2.3");
        assert_eq!(virtual_key.name(), "example");
        assert_eq!(virtual_key.version(), "0.0.0");
    }

    #[test]
    fn stable_template_and_instance_keys_use_semantic_fields_instead_of_internal_ids() {
        let cone = StableConeKey::new("demo", "0.1.0");
        let template = StableTemplateKey::new(StableDefKey::new(
            cone,
            StableDefNamespace::Fun,
            "pkg.main.id",
            "generic_fun",
            Some("fun|T|Int|Unit".to_string()),
        ));
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let instance = StableInstanceKey::from_type_arguments(
            template.clone(),
            &types,
            &[builtins.int],
            &[EffectRow::new(Vec::new())],
            &NoTypeParamResolver,
        )
        .unwrap();
        let template_text = template.canonical_text();
        let instance_text = instance.canonical_text();

        assert!(template_text.contains("pkg.main.id"));
        assert!(instance_text.contains("V(Int)"));
        assert!(instance_text.contains("E()"));
        assert!(!instance_text.contains("TypeId"));
        assert!(!instance_text.contains("decl_span"));
    }

    #[test]
    fn canonical_callable_signature_key_depends_on_canonical_type_text_and_generic_arity() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();
        let param = TypeParamType {
            name: "T".to_string(),
            decl_file: std::path::PathBuf::from("<sig>"),
            decl_span: Span::new(1, 2),
        };
        let param_ty = types.ty_param(param.clone());
        let callable_ty = types.ty_function(
            None,
            vec![param_ty],
            builtins.unit,
            EffectRow::pure(),
            false,
        );
        let resolver = HashMap::from([(param, StableTypeParamKey::new("pkg.owner.fun", 0))]);

        let base = canonical_callable_signature_key(&types, callable_ty, 0, 1, 0, &resolver)
            .expect("callable signature key should encode canonical type text");
        let different_arity =
            canonical_callable_signature_key(&types, callable_ty, 1, 0, 0, &resolver)
                .expect("generic arity must participate in callable signature key");

        assert!(base.contains("F(-;[P(pkg.owner.fun#0)]->V(Unit)/E())"));
        assert!(!base.contains("TypeId"));
        assert_ne!(base, different_arity);
    }

    #[test]
    fn canonical_property_getter_signature_key_depends_on_owner_arity() {
        let mut types = TypeStore::new();
        let builtins = types.intern_builtins();

        let base =
            canonical_property_getter_signature_key(&types, builtins.int, 1, &NoTypeParamResolver)
                .expect("property getter signature key should encode canonical return type");
        let different_owner_arity =
            canonical_property_getter_signature_key(&types, builtins.int, 0, &NoTypeParamResolver)
                .expect("owner generic arity must participate in getter signature key");

        assert!(base.contains("V(Int)"));
        assert_ne!(base, different_owner_arity);
    }

    #[test]
    fn abi_and_private_manglers_emit_expected_namespaces() {
        let def = StableDefKey::new(
            StableConeKey::new("demo", "0.1.0"),
            StableDefNamespace::Fun,
            "pkg.main",
            "top_level_fun",
            None,
        );
        let closure = StableClosureKey::new(&def, "$lambda0");

        let abi_fun = AbiMangler.fun_symbol(&def);
        let abi_global = AbiMangler.global_symbol(&def);
        let abi_type = AbiMangler.type_symbol(&def);
        let private = PrivateSymbolMangler.mangle("closure_step_adapter", &closure);

        assert!(abi_fun.starts_with("__scoop_abi0_fun__pkg_main__h"));
        assert!(abi_global.starts_with("__scoop_abi0_global__pkg_main__h"));
        assert!(abi_type.starts_with("__scoop_abi0_type__pkg_main__h"));
        assert!(private.starts_with("__scoop_priv0__closure_step_adapter__h"));
        assert_ne!(abi_fun, abi_global);
        assert_ne!(abi_fun, abi_type);
    }

    #[test]
    fn stable_template_symbol_suffix_depends_on_stable_template_key() {
        let base = StableDefKey::new(
            StableConeKey::new("demo", "0.1.0"),
            StableDefNamespace::Fun,
            "pkg.id",
            "generic_fun",
            Some("sig-a".to_string()),
        );
        let same = StableTemplateKey::new(base.clone());
        let different_signature = StableTemplateKey::new(StableDefKey::new(
            StableConeKey::new("demo", "0.1.0"),
            StableDefNamespace::Fun,
            "pkg.id",
            "generic_fun",
            Some("sig-b".to_string()),
        ));
        let different_cone = StableTemplateKey::new(StableDefKey::new(
            StableConeKey::new("demo-next", "0.1.0"),
            StableDefNamespace::Fun,
            "pkg.id",
            "generic_fun",
            Some("sig-a".to_string()),
        ));

        assert_eq!(
            stable_template_symbol_suffix(&same),
            stable_template_symbol_suffix(&StableTemplateKey::new(base))
        );
        assert_ne!(
            stable_template_symbol_suffix(&same),
            stable_template_symbol_suffix(&different_signature)
        );
        assert_ne!(
            stable_template_symbol_suffix(&same),
            stable_template_symbol_suffix(&different_cone)
        );
    }

    #[test]
    fn stable_local_label_uses_dump_scope_over_key_canonical_text() {
        let owner = StableDefKey::new(
            StableConeKey::new("demo", "0.1.0"),
            StableDefNamespace::Fun,
            "pkg.main",
            "top_level_fun",
            None,
        );
        let site = StableCallSiteKey::new(&owner, "src/main.scoop", Span::new(3, 9), "call");

        assert_eq!(
            stable_local_label("site", &site),
            format!(
                "site#h{}",
                stable_hash128_hex(StableHashScope::DumpV0, &site.canonical_text())
            )
        );
    }
}
