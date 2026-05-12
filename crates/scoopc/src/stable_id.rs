//! Shared stable-id primitives.
//!
//! This module is the single entry point for canonical type/effect encoding,
//! versioned hashing, and short stable dump labels. Callers must feed semantic
//! keys or canonical text here instead of reusing `TypeStore::display()`, raw
//! `Debug` output, or path/span text as the authoritative identity input.

use std::collections::HashMap;

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::ty::{
    EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeParamType, TypeStore, UnionType,
    ValueTypeKind,
};

const MAX_CANONICAL_DEPTH: usize = 64;

/// Versioned hash scopes shared by ABI, private symbols, RTTI, and dumps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StableHashScope {
    AbiV0,
    PrivateV0,
    RttiV0,
    DumpV0,
}

impl StableHashScope {
    /// Returns the fixed textual prefix mandated by `STABLE_ID.md`.
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::AbiV0 => "abi0:",
            Self::PrivateV0 => "priv0:",
            Self::RttiV0 => "rtti0:",
            Self::DumpV0 => "dump0:",
        }
    }
}

/// Stable type-parameter identity used by canonical type text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
#[derive(Debug, Default, Clone, Copy)]
pub struct NoTypeParamResolver;

impl StableTypeParamResolver for NoTypeParamResolver {
    fn resolve(&self, _: &TypeParamType) -> Option<StableTypeParamKey> {
        None
    }
}

/// Errors raised while building canonical type/effect text.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CanonicalEncodingError {
    #[error("missing stable type parameter key for `{param_name}`")]
    MissingTypeParamKey { param_name: String },
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
    encoder.encode_effect_row(row, 0)
}

/// Hashes canonical text with a fixed version prefix using SHA-256.
pub fn stable_digest(scope: StableHashScope, canonical_text: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(scope.prefix().as_bytes());
    hasher.update(canonical_text.as_bytes());
    hasher.finalize().into()
}

/// Returns the linker-visible 128-bit truncated hash as lowercase hex.
pub fn stable_hash128_hex(scope: StableHashScope, canonical_text: &str) -> String {
    let digest = stable_digest(scope, canonical_text);
    hex_lower(&digest[..16])
}

/// Returns the runtime-only 64-bit truncated hash.
pub fn stable_hash64(scope: StableHashScope, canonical_text: &str) -> u64 {
    let digest = stable_digest(scope, canonical_text);
    let bytes: [u8; 8] = digest[..8]
        .try_into()
        .expect("sha256 output is always 32 bytes");
    u64::from_le_bytes(bytes)
}

/// Builds a short dump label from a semantic role plus canonical text.
pub fn stable_dump_label(role: &str, canonical_text: &str) -> String {
    format!(
        "{role}#h{}",
        stable_hash128_hex(StableHashScope::DumpV0, canonical_text)
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
                let row = self.encode_effect_row(&fun.effects, depth + 1)?;
                let closed = if fun.effects_closed { "!" } else { "" };
                format!("F({receiver};[{params}]->{return_ty}/{row}{closed})")
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
        let mut encoded = format!("N({}", nominal.fqn);
        if !nominal.args.is_empty() || nominal.eff.is_some() {
            encoded.push('<');
            let args = self.encode_type_list(&nominal.args, depth + 1)?;
            if !args.is_empty() {
                encoded.push_str(&args);
            }
            if let Some(eff) = &nominal.eff {
                if !args.is_empty() {
                    encoded.push(';');
                }
                encoded.push_str("eff=");
                encoded.push_str(&self.encode_effect_row(eff, depth + 1)?);
            }
            encoded.push('>');
        }
        encoded.push(')');
        Ok(encoded)
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
        depth: usize,
    ) -> Result<String, CanonicalEncodingError> {
        let mut terms = row
            .terms
            .iter()
            .copied()
            .map(|term| self.encode_type(term, depth + 1))
            .collect::<Result<Vec<_>, _>>()?;
        terms.sort();
        terms.dedup();
        Ok(format!("E({})", terms.join(",")))
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

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::span::Span;

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
}
