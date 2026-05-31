//! Frontend-owned monomorphization request keys.

use std::fmt;
use std::path::PathBuf;

use crate::span::Span;
use crate::stable_id::{StableInstanceKey, StableTemplateKey};
use crate::ty::{EffectRow, TypeId};

/// Stable reference to a monomorphization target observed by the frontend.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MonomorphSymbol {
    pub fqn: String,
    pub decl_file: PathBuf,
    pub decl_span: Span,
}

impl fmt::Debug for MonomorphSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}@{}:{:?}",
            self.fqn,
            self.decl_file.display(),
            self.decl_span
        )
    }
}

/// Frontend request key for a concrete generic instance.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MonomorphKey {
    pub symbol: MonomorphSymbol,
    pub stable_template_key: Option<StableTemplateKey>,
    pub stable_instance_key: Option<StableInstanceKey>,
    pub type_args: Vec<TypeId>,
    pub eff_args: Vec<EffectRow>,
}

impl MonomorphKey {
    pub fn with_stable_identity(
        mut self,
        stable_template_key: StableTemplateKey,
        stable_instance_key: StableInstanceKey,
    ) -> Self {
        self.stable_template_key = Some(stable_template_key);
        self.stable_instance_key = Some(stable_instance_key);
        self
    }
}

impl fmt::Debug for MonomorphKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonomorphKey")
            .field("symbol", &self.symbol)
            .field("stable_template_key", &self.stable_template_key)
            .field("stable_instance_key", &self.stable_instance_key)
            .field("type_args", &TypeIdList(&self.type_args))
            .field("eff_args", &EffectRowList(&self.eff_args))
            .finish()
    }
}

/// A monomorphization request with its call-site source anchor.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct MonomorphRequest {
    pub key: MonomorphKey,
    pub request_source_path: PathBuf,
    pub call_span: Span,
}

impl MonomorphRequest {
    pub fn new(key: MonomorphKey, request_source_path: PathBuf, call_span: Span) -> Self {
        Self {
            key,
            request_source_path,
            call_span,
        }
    }
}

impl fmt::Debug for MonomorphRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MonomorphRequest")
            .field("key", &self.key)
            .field("request_source_path", &self.request_source_path)
            .field("call_span", &self.call_span)
            .finish()
    }
}

struct TypeIdList<'a>(&'a [TypeId]);

impl fmt::Debug for TypeIdList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().copied().map(TypeIdRepr))
            .finish()
    }
}

struct TypeIdRepr(TypeId);

impl fmt::Debug for TypeIdRepr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "t{}", self.0.as_u32())
    }
}

struct EffectRowList<'a>(&'a [EffectRow]);

impl fmt::Debug for EffectRowList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list()
            .entries(self.0.iter().map(EffectRowRepr))
            .finish()
    }
}

struct EffectRowRepr<'a>(&'a EffectRow);

impl fmt::Debug for EffectRowRepr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_pure() {
            return write!(f, "Pure");
        }
        f.debug_list()
            .entries(self.0.terms.iter().copied().map(TypeIdRepr))
            .finish()
    }
}
