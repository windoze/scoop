//! [`TypeEnv`]：typecheck 的类型查询环境。
//!
//! 持有 [`TypeStore`]（类型存储）、对 resolve [`Index`](crate::resolve::Index) 与
//! [`Interner`] 的引用，并提供**内建类型表**（标量 / String / Unit / Nothing）。
//! nominal 类型（class/struct/enum/...）的 ref/value 由 [`Index::category`] 决定。

use std::collections::{HashMap, HashSet};

use scoop2_base::diag::DiagnosticSink;
use scoop2_base::{FileId, Interner, Symbol};

use crate::resolve::imports::ImportTable;
use crate::resolve::index::Index;
use crate::syntax::ast::{File, ItemKind, TypeMember, TypeMemberKind, TypeParamList, ValBinding};
use crate::ty::{EffectRow, TypeId, TypeStore};

use super::lower::TypeLowering;

/// 一个函数签名（已降级；M2 用于单候选调用）。
#[derive(Clone, Debug)]
pub struct Signature {
    pub params: Vec<TypeId>,
    pub return_ty: TypeId,
    /// 类型参数个数（>0 表示泛型；M3 才支持实例化）。
    pub type_param_count: usize,
    /// 各类型参数的声明 bound（降级后的类型；无 bound 为 None）。M3 泛型重载特异性用。
    pub type_param_bounds: Vec<Option<TypeId>>,
    /// 参数名（与 params 等长）。
    pub param_names: Vec<Symbol>,
    /// 各参数是否有默认值（与 params 等长）。
    pub has_defaults: Vec<bool>,
    /// 各参数默认值表达式的克隆（与 params 等长；None = 无默认值）。
    /// 供 HIR 层在调用点填充缺失参数时 lower / 存储默认表达式。
    pub default_exprs: Vec<Option<crate::syntax::ast::Expr>>,
    /// 是否有 vararg 参数（最后一个参数为 `vararg`，可接收任意多余位置实参）。
    pub has_vararg: bool,
    /// 声明处修饰符（open/abstract/override/final 等；M6 override 匹配用）。
    pub modifiers: crate::resolve::symbol::ModifierSet,
    /// 声明的 effect 行（M6 override effect containment 用）。
    pub effect: Option<crate::syntax::ast::EffectRowExpr>,
    /// effect 行参数的 id（`<eff E>` 中的 E；无 eff 参数为 None）。
    /// 供 effect row 快照时把 E 写入 `EffectRow::tail = Var(id)`。
    pub eff_param_id: Option<crate::ty::TypeParamId>,
    /// 是否带函数体（区分 interface default 方法 / abstract 方法；M6 用）。
    pub has_body: bool,
    /// 声明 span（M3 构造器重载 related 标签用；顶层/成员默认 default）。
    pub decl_span: scoop2_base::Span,
    /// 声明所在文件的 FileId（跨文件诊断渲染用）。
    pub decl_file: scoop2_base::FileId,
}

/// 顶层函数的注解属性（release-hook 等 cross-reference 校验用）。
#[derive(Clone, Copy, Default, Debug)]
pub struct FunAttrs {
    pub is_nogc: bool,
    /// `@Extern` 且 ABI 为缺省或 `"c"`（native C ABI leaf）。
    pub is_native_extern: bool,
}

/// typecheck 类型环境。
pub struct TypeEnv<'i> {
    pub store: TypeStore,
    pub index: &'i Index,
    pub interner: &'i Interner,
    /// FQN → 函数签名重载集（顶层函数；M2）。
    signatures: HashMap<Symbol, Vec<Signature>>,
    /// 类型 FQN → (成员名 → 成员类型)。属性 / 字段（含主构造 param-property）。
    members: HashMap<Symbol, HashMap<Symbol, TypeId>>,
    /// 类型 FQN → 成员名列表（按声明顺序；与 `members` 同步填充）。
    /// 字段布局 / 偏移计算的确定性来源（`members` HashMap 迭代序不确定）。
    member_order: HashMap<Symbol, Vec<Symbol>>,
    /// 类型 FQN → 不可变（`val`）属性名集合（赋值目标可变性检查用）。
    immutable_members: HashMap<Symbol, HashSet<Symbol>>,
    /// 类型 FQN → 主构造参数类型列表。
    ctors: HashMap<Symbol, Vec<TypeId>>,
    /// 类型 FQN → 次构造器签名重载集（M3 构造器重载决议用）。
    ctor_signatures: HashMap<Symbol, Vec<Signature>>,
    /// 类型 FQN → (方法名 → 签名重载集)。成员函数 / 扩展。
    member_signatures: HashMap<Symbol, HashMap<Symbol, Vec<Signature>>>,
    /// 类型 FQN → 成员函数名列表（按声明顺序；与 `member_signatures` 同步填充）。
    /// vtable / itable slot 分配的确定性来源（内层 HashMap 迭代序不确定）。
    member_fun_order: HashMap<Symbol, Vec<Symbol>>,
    /// 带 `@CLayout` 注解的 struct FQN 集合（native `@Extern` ABI 允许的 nominal 值类型）。
    clayout_structs: HashSet<Symbol>,
    /// 顶层函数 FQN → 注解属性（release-hook cross-reference 校验用）。
    fun_attrs: HashMap<Symbol, FunAttrs>,
    /// typealias FQN → (RHS TypeRef, 类型参数名序列)。
    type_aliases: HashMap<Symbol, (crate::syntax::ast::TypeRef, Vec<Symbol>)>,
    /// 类型 FQN → (类型参数名序列, where 约束 [(参数名, bound)])。
    #[allow(clippy::type_complexity)]
    type_constraints:
        HashMap<Symbol, (Vec<Symbol>, Vec<(Symbol, crate::syntax::ast::GenericBound)>)>,
    /// 顶层 val/var 简单名 → 已降级类型（供表达式引用解析）。
    top_level_vals: HashMap<Symbol, TypeId>,
    /// enum FQN → variant 名列表（when 穷尽性检查用）。
    pub enum_variants: HashMap<Symbol, Vec<Symbol>>,
    /// (enum FQN, variant 名) → payload 字段数（pattern arity 校验用）。
    /// 内建 Option 的 Some/None 在 typecheck 侧特判（不登记在此）。
    enum_variant_arities: HashMap<(Symbol, Symbol), usize>,
    /// 声明了 eff 形参的类型 FQN 集合（use-site eff 实参合法性检查用）。
    eff_param_types: HashSet<Symbol>,
    /// 类型 FQN → 直接超类型列表（(超类型 FQN, 类型实参 TypeIds)）。
    /// 用于 where 约束中参数化 bound 的类型实参检查。
    supertype_instances: HashMap<Symbol, Vec<(Symbol, Vec<TypeId>)>>,
    /// class FQN → 主构造器参数布局（含非属性参数；继承构造链展开用）。
    pub class_ctor_params: HashMap<Symbol, Vec<crate::hir::ClassCtorParamInfo>>,
    /// class FQN → `: Super(args)` 委托（可静态解析时记录）。
    pub super_ctor_delegations: HashMap<Symbol, crate::hir::SuperCtorDelegation>,
}

impl<'i> TypeEnv<'i> {
    pub fn new(index: &'i Index, interner: &'i Interner) -> Self {
        Self {
            store: TypeStore::new(),
            index,
            interner,
            signatures: HashMap::new(),
            members: HashMap::new(),
            member_order: HashMap::new(),
            immutable_members: HashMap::new(),
            ctors: HashMap::new(),
            ctor_signatures: HashMap::new(),
            member_signatures: HashMap::new(),
            member_fun_order: HashMap::new(),
            clayout_structs: HashSet::new(),
            fun_attrs: HashMap::new(),
            type_aliases: HashMap::new(),
            type_constraints: HashMap::new(),
            top_level_vals: HashMap::new(),
            enum_variants: HashMap::new(),
            enum_variant_arities: HashMap::new(),
            eff_param_types: HashSet::new(),
            supertype_instances: HashMap::new(),
            class_ctor_params: HashMap::new(),
            super_ctor_delegations: HashMap::new(),
        }
    }

    /// nominal 子类型（传递超类型链）。
    pub fn fqn_is_subtype(&self, sub: Symbol, sup: Symbol) -> bool {
        if sub == sup || self.fqn_same_simple_name(sub, sup) {
            return true;
        }
        self.index
            .supertypes_of(sub)
            .iter()
            .any(|&s| self.fqn_is_subtype(s, sup))
    }

    /// 两个 FQN 是否末段名相同（simple name 匹配）。
    /// 用于超类型 FQN 解析不精确时的回退匹配（collect 阶段超类型按 package 前缀解析，
    /// 跨 import 的超类型可能 FQN 不精确但 simple name 一致）。
    fn fqn_same_simple_name(&self, a: Symbol, b: Symbol) -> bool {
        let ta = self.interner.resolve(a);
        let tb = self.interner.resolve(b);
        ta.rsplit('.').next() == tb.rsplit('.').next() && !ta.is_empty()
    }

    /// 顶层 val 类型查询（表达式引用解析用）。
    pub fn top_level_val(&self, name: Symbol) -> Option<TypeId> {
        self.top_level_vals.get(&name).copied()
    }

    /// 顶层 val 的 FQN 查询（赋值目标 record_assign_place 用）。
    /// 返回 name symbol 自身（与 value_ref 记录的 FQN 一致——collect 阶段
    /// top_level_vals 以 simple name symbol 为键，存储侧按同 symbol 查 global）。
    pub fn top_level_val_fqn(&self, name: Symbol) -> Option<Symbol> {
        if self.top_level_vals.contains_key(&name) {
            Some(name)
        } else {
            None
        }
    }

    /// enum variant 列表查询（when 穷尽性用）。
    pub fn enum_variants(&self, fqn: Symbol) -> Option<&[Symbol]> {
        self.enum_variants.get(&fqn).map(|v| v.as_slice())
    }

    /// 统计所有 enum 中名为 `variant_name_text` 的 variant 数量（消歧判定用）。
    /// 多个 enum 拥有同名 variant（如 Some/None）时，构造应留给期望类型消歧。
    pub fn count_variants_named(&self, variant_name_text: &str) -> usize {
        let mut count = 0usize;
        for variants in self.enum_variants.values() {
            for &v in variants {
                if self.interner.resolve(v) == variant_name_text {
                    count += 1;
                }
            }
        }
        // 内建 Option.Some / Option.None 始终存在。
        if variant_name_text == "Some" || variant_name_text == "None" {
            count += 1;
        }
        count
    }

    /// (enum FQN, variant 名) → payload 字段数（pattern arity 校验用）。
    /// 内建 Option 由 typecheck 侧特判（不登记在此）。
    pub fn enum_variant_arity(&self, enum_fqn: Symbol, variant: Symbol) -> Option<usize> {
        self.enum_variant_arities.get(&(enum_fqn, variant)).copied()
    }

    /// 类型是否声明了 eff 形参。
    pub fn has_eff_param(&self, fqn: Symbol) -> bool {
        self.eff_param_types.contains(&fqn)
    }

    /// 类型的直接超类型实例化列表（FQN + 类型实参）。
    pub fn supertype_instances(&self, fqn: Symbol) -> Option<&[(Symbol, Vec<TypeId>)]> {
        self.supertype_instances.get(&fqn).map(|v| v.as_slice())
    }

    /// 注册类型的直接超类型实例化（用于 where 约束参数化 bound 检查）。
    pub fn register_supertype_instances(
        &mut self,
        fqn: Symbol,
        supertypes: Vec<(Symbol, Vec<TypeId>)>,
    ) {
        self.supertype_instances.insert(fqn, supertypes);
    }

    /// 类型约束查询（where-satisfaction 用）。
    #[allow(clippy::type_complexity)]
    pub fn type_constraints(
        &self,
        fqn: Symbol,
    ) -> Option<(&[Symbol], &[(Symbol, crate::syntax::ast::GenericBound)])> {
        self.type_constraints
            .get(&fqn)
            .map(|(n, c)| (n.as_slice(), c.as_slice()))
    }

    /// 搜索所有注册的 type_constraints，找到名为 `param_name` 的类型参数的所有 Type bound。
    ///
    /// 注意：这是**跨文件全局按名搜索**（任意 owner 的同名约束都会命中），
    /// 仅保留给无当前声明上下文的调用方；函数体检查应改用
    /// [`Self::type_param_bounds_for`]（按当前声明 owner 作用域），避免
    /// 把别的文件里同名类型参数的约束泄漏进当前上下文。
    pub fn find_type_param_bounds(
        &mut self,
        param_name: Symbol,
    ) -> Vec<crate::syntax::ast::TypeRef> {
        self.find_type_param_bounds_immutable(param_name)
    }

    /// 按 owner FQN 列表查找名为 `param_name` 的类型参数的 Type bound TypeRefs。
    ///
    /// 只搜索 `owners`（当前函数 / 所属类型）注册的 where 约束，避免跨文件
    /// 同名类型参数的约束互相泄漏（泄漏会把外文件的 bound TypeRef 拿到当前
    /// 文件的 package 上下文里降级，产生非确定的 unresolved_type_ref）。
    pub fn type_param_bounds_for(
        &self,
        owners: &[Symbol],
        param_name: Symbol,
    ) -> Vec<crate::syntax::ast::TypeRef> {
        let mut result = Vec::new();
        for owner in owners {
            if let Some((_, cons)) = self.type_constraints.get(owner) {
                for (cname, bound) in cons {
                    if *cname == param_name
                        && let crate::syntax::ast::GenericBound::Type(t) = bound
                    {
                        result.push(t.clone());
                    }
                }
            }
        }
        result
    }

    /// `find_type_param_bounds` 的不可变版本（供借用 `&self` 的调用方使用）。
    pub fn find_type_param_bounds_immutable(
        &self,
        param_name: Symbol,
    ) -> Vec<crate::syntax::ast::TypeRef> {
        let mut result = Vec::new();
        for (_, cons) in self.type_constraints.values() {
            for (cname, bound) in cons {
                if *cname == param_name
                    && let crate::syntax::ast::GenericBound::Type(t) = bound
                {
                    result.push(t.clone());
                }
            }
        }
        result
    }

    /// typealias 查询（展开用）。
    pub fn type_alias(&self, fqn: Symbol) -> Option<(&crate::syntax::ast::TypeRef, &[Symbol])> {
        self.type_aliases.get(&fqn).map(|(t, p)| (t, p.as_slice()))
    }

    /// `@CLayout` struct 查询（native `@Extern` ABI 用）。
    pub fn is_clayout_struct(&self, fqn: Symbol) -> bool {
        self.clayout_structs.contains(&fqn)
    }

    /// 顶层函数注解属性查询（release-hook 用）。
    pub fn fun_attrs(&self, fqn: Symbol) -> Option<FunAttrs> {
        self.fun_attrs.get(&fqn).copied()
    }

    /// 顶层函数（非扩展、非成员）的签名重载集。
    pub fn signatures(&self, fqn: Symbol) -> Option<&[Signature]> {
        self.signatures.get(&fqn).map(|v| v.as_slice())
    }

    /// 类型上的成员函数签名重载集（`type_fqn.method_name`）。
    pub fn member_signatures(&self, type_fqn: Symbol, method: Symbol) -> Option<&[Signature]> {
        self.member_signatures
            .get(&type_fqn)
            .and_then(|m| m.get(&method).map(|v| v.as_slice()))
    }

    /// 成员方法签名（含继承）：遍历超类型链，收集所有层级的同名方法签名。
    /// 用于方法重载决议——子类可继承父类的方法重载。
    pub fn member_signatures_with_inherited(
        &self,
        type_fqn: Symbol,
        method: Symbol,
    ) -> Vec<Signature> {
        self.member_signatures_with_owners(type_fqn, method)
            .into_iter()
            .map(|(_, s)| s)
            .collect()
    }

    /// 同 [`member_signatures_with_inherited`]，但保留每个签名的声明者 FQN
    /// （用于扩展方法 receiver 特异性比较）。
    pub fn member_signatures_with_owners(
        &self,
        type_fqn: Symbol,
        method: Symbol,
    ) -> Vec<(Symbol, Signature)> {
        let mut all: Vec<(Symbol, Signature)> = Vec::new();
        let mut visited: std::collections::HashSet<Symbol> = std::collections::HashSet::new();
        let mut current = Some(type_fqn);
        while let Some(fqn) = current {
            if !visited.insert(fqn) {
                break;
            }
            if let Some(sigs) = self.member_signatures(fqn, method) {
                all.extend(sigs.iter().map(|s| (fqn, s.clone())));
            }
            // 沿超类型链上溯。
            let supertypes = self.index.supertypes_of(fqn);
            if let Some(&sup) = supertypes.first() {
                current = Some(sup);
            } else {
                // 无显式超类型链：引用类型隐式继承 Any（prelude 根引用类型），
                // 使 Any 的扩展方法对所有引用类型可见。
                let fqn_text = self.interner.resolve(fqn);
                let is_ref_type = fqn_text.starts_with("scoop.core.")
                    && !matches!(
                        fqn_text,
                        "scoop.core.Int"
                            | "scoop.core.UInt"
                            | "scoop.core.Bool"
                            | "scoop.core.Char"
                            | "scoop.core.Float32"
                            | "scoop.core.Float64"
                    );
                if is_ref_type
                    && let Some(any_fqn) = self.interner.get("scoop.core.Any")
                    && any_fqn != fqn
                {
                    current = Some(any_fqn);
                } else {
                    current = None;
                }
            }
        }
        all
    }

    /// 类型的所有成员方法名 → 签名集（M6 interface 成员枚举 / impl 完整性用）。
    pub fn member_method_table(
        &self,
        type_fqn: Symbol,
    ) -> Option<&HashMap<Symbol, Vec<Signature>>> {
        self.member_signatures.get(&type_fqn)
    }

    /// 类型的主构造参数类型列表。
    pub fn ctor_params(&self, fqn: Symbol) -> Option<&[TypeId]> {
        self.ctors.get(&fqn).map(|v| v.as_slice())
    }

    /// 类型的次构造器签名重载集。
    pub fn ctor_signatures(&self, fqn: Symbol) -> Option<&[Signature]> {
        self.ctor_signatures.get(&fqn).map(|v| v.as_slice())
    }

    /// 类型的属性 / 字段成员类型（`type_fqn.member_name`）。
    pub fn member_type(&self, type_fqn: Symbol, member_name: Symbol) -> Option<TypeId> {
        self.members
            .get(&type_fqn)
            .and_then(|m| m.get(&member_name).copied())
    }

    /// 类型的全部字段成员类型（GC-free 递归校验用）。
    pub fn member_types(&self, type_fqn: Symbol) -> Option<&HashMap<Symbol, TypeId>> {
        self.members.get(&type_fqn)
    }

    /// 类型的字段成员类型列表（按声明序）。
    ///
    /// enum variant payload 字段（`<enum>.<variant>` 名义下登记）/ struct 字段的
    /// 确定性顺序来源（`members` HashMap 迭代序不确定）；`member_order` 缺失时
    /// 回退按成员名排序。
    pub fn ordered_member_types(&self, type_fqn: Symbol) -> Vec<TypeId> {
        let Some(members) = self.members.get(&type_fqn) else {
            return Vec::new();
        };
        match self.member_order.get(&type_fqn) {
            Some(order) => order
                .iter()
                .filter_map(|name| members.get(name).copied())
                .collect(),
            None => {
                let mut sorted: Vec<(Symbol, TypeId)> =
                    members.iter().map(|(&n, &t)| (n, t)).collect();
                sorted.sort_by(|a, b| self.interner.resolve(a.0).cmp(self.interner.resolve(b.0)));
                sorted.into_iter().map(|(_, t)| t).collect()
            }
        }
    }

    /// 成员是否为不可变（`val`）属性（赋值目标可变性检查）。
    pub fn is_immutable_member(&self, type_fqn: Symbol, member_name: Symbol) -> bool {
        self.immutable_members
            .get(&type_fqn)
            .is_some_and(|s| s.contains(&member_name))
    }

    /// 内建标量 / String / Unit / Nothing 名字 → [`TypeId`]。
    /// 接受短名（`Int`）或全限定（`scoop.core.Int`）。
    /// 不含 `Option`/`Array`——它们是 prelude nominal，由 [`super::lower`] 经 Index 解析。
    pub fn builtin(&mut self, name: &str) -> Option<TypeId> {
        let n = name.strip_prefix("scoop.core.").unwrap_or(name);
        let s = &mut self.store;
        Some(match n {
            "Int" => s.int(),
            "UInt" | "UIntPtr" => s.uint(),
            "Bool" => s.bool(),
            "Char" => s.char(),
            "Float64" | "Double" => s.float64(),
            "Float32" => s.float32(),
            "Int8" => s.int_n(8),
            "Int16" | "Short" => s.int_n(16),
            "Int32" => s.int_n(32),
            "Int64" | "Long" => s.int_n(64),
            "UInt8" | "Byte" => s.uint_n(8),
            "UInt16" | "UShort" => s.uint_n(16),
            "UInt32" => s.uint_n(32),
            "UInt64" | "ULong" => s.uint_n(64),
            "String" => s.string(),
            "Unit" => s.unit(),
            "Nothing" => s.nothing(),
            _ => return None,
        })
    }

    /// nominal 类型是否引用类型（按 [`Index::category`]）。
    pub fn is_reference_nominal(&self, fqn: Symbol) -> bool {
        self.index.category(fqn).is_some_and(|c| c.is_reference())
    }

    /// 把 typecheck 产出的类型数据 move 进自包含的 [`crate::hir::TypedHir`]。
    ///
    /// 消费 `self`（取出 `store` / 签名 / 成员 / 顶层 val / enum variant 表），
    /// 并把 per-file `expr_types` 与 interner 副本一并装入。调用后 `self` 不可用。
    pub fn into_typed_hir(
        self,
        interner: scoop2_base::Interner,
        files: Vec<crate::hir::TypedFile>,
    ) -> crate::hir::TypedHir {
        use crate::hir::{TypedHir, TypedSignature};
        let index = self.index;
        let interner_ref = &interner;
        let index_ref = &index;
        // 把私有 Signature 表转换为公开 TypedSignature 表。
        let convert_sigs = |sigs: Vec<Signature>, store: &mut TypeStore| -> Vec<TypedSignature> {
            sigs.into_iter()
                .map(|s| TypedSignature {
                    param_types: s.params,
                    return_ty: s.return_ty,
                    type_param_count: s.type_param_count,
                    param_names: s.param_names,
                    has_defaults: s.has_defaults,
                    default_exprs: s.default_exprs,
                    effect_row: resolve_signature_effect_row(
                        store,
                        index_ref,
                        interner_ref,
                        s.effect.as_ref(),
                        s.eff_param_id,
                    ),
                    has_vararg: s.has_vararg,
                    decl_span: s.decl_span,
                    decl_file: s.decl_file,
                })
                .collect()
        };
        let mut store = self.store;
        // 收集所有 interface FQN，供 MIR 区分 itable vs class vtable 分发通道。
        let interface_fqns: std::collections::HashSet<scoop2_base::Symbol> = index
            .categories_iter()
            .filter(|(_, c)| *c == crate::resolve::symbol::NominalCategory::Interface)
            .map(|(fqn, _)| fqn)
            .collect();
        // 收集所有 class FQN（含 final/open/abstract），供 MIR lower 判定成员函数 receiver 是 ref。
        let class_fqns: std::collections::HashSet<scoop2_base::Symbol> = index
            .categories_iter()
            .filter(|(_, c)| *c == crate::resolve::symbol::NominalCategory::Class)
            .map(|(fqn, _)| fqn)
            .collect();
        // 收集所有可继承的 class FQN（`open`/`abstract`），供 MIR 去虚化 pass 判断
        // ref 类型接收者是否 final。补集 = 具体 class（不可继承 → 方法不可 override）。
        let extensible_class_fqns: std::collections::HashSet<scoop2_base::Symbol> = index
            .categories_iter()
            .filter(|(_, c)| *c == crate::resolve::symbol::NominalCategory::Class)
            .filter_map(|(fqn, _)| {
                let decl = index.lookup_type(fqn)?;
                let ms = decl.modifiers;
                if ms.is_open() || ms.is_abstract() {
                    Some(fqn)
                } else {
                    None
                }
            })
            .collect();
        // 收集超类型 → 直接子类型映射（反转 index.supertypes），供 MIR 去虚化 CHA。
        let mut direct_subtypes: std::collections::HashMap<
            scoop2_base::Symbol,
            Vec<scoop2_base::Symbol>,
        > = std::collections::HashMap::new();
        let mut supertypes: std::collections::HashMap<
            scoop2_base::Symbol,
            Vec<scoop2_base::Symbol>,
        > = std::collections::HashMap::new();
        for (child, supers) in index.supertypes_iter() {
            for &sup in supers {
                direct_subtypes.entry(sup).or_default().push(child);
            }
            supertypes.insert(child, supers.to_vec());
        }
        // 子类型列表按 Symbol 排序：来源是 HashMap 迭代（顺序不定），而 CHA 只做
        // 成员判定——排序消除迭代序对下游字节流的影响（PLAN.md C7）。
        for children in direct_subtypes.values_mut() {
            children.sort_unstable();
        }
        // 签名表按 Symbol 排序后转换：convert_sigs 会铸造 effect-row TypeId，
        // 迭代序必须确定（PLAN.md C7——HashMap 序不得影响 TypeStore 分配序）。
        let top_level_funs = {
            let mut entries: Vec<_> = self.signatures.into_iter().collect();
            entries.sort_by_key(|(k, _)| *k);
            entries
                .into_iter()
                .map(|(k, v)| (k, convert_sigs(v, &mut store)))
                .collect()
        };
        let member_funs = {
            let mut entries: Vec<_> = self
                .member_signatures
                .into_iter()
                .map(|(k, m)| {
                    let mut inner: Vec<_> = m.into_iter().collect();
                    inner.sort_by_key(|(mk, _)| *mk);
                    (
                        k,
                        inner
                            .into_iter()
                            .map(|(mk, v)| (mk, convert_sigs(v, &mut store)))
                            .collect(),
                    )
                })
                .collect();
            entries.sort_by_key(|(k, _)| *k);
            entries.into_iter().collect()
        };
        let ctor_signatures = {
            let mut entries: Vec<_> = self.ctor_signatures.into_iter().collect();
            entries.sort_by_key(|(k, _)| *k);
            entries
                .into_iter()
                .map(|(k, v)| (k, convert_sigs(v, &mut store)))
                .collect()
        };
        let type_constraints = self
            .type_constraints
            .into_iter()
            .map(|(k, (params, cons))| {
                (
                    k,
                    crate::hir::TypeConstraintsSnapshot {
                        type_params: params,
                        constraints: cons,
                    },
                )
            })
            .collect();
        // 构建 `HashMap<TypeId, TypeInfo>`：把上述按 FQN 分散的声明信息合并为按
        // `TypeId` 索引的单条信息（freeze 边界）。当前与旧字段并存（增量迁移）。
        let type_infos = build_type_infos(
            &mut store,
            index,
            interner_ref,
            &member_funs,
            &self.member_fun_order,
            &self.members,
            &self.member_order,
            &ctor_signatures,
            &self.enum_variants,
            &extensible_class_fqns,
            &self.class_ctor_params,
            &self.super_ctor_delegations,
        );
        let lang_items = crate::hir::lang_items::LangItems::resolve(&interner);
        let mut hir = TypedHir {
            store,
            interner,
            top_level_funs,
            member_funs,
            member_fun_order: self.member_fun_order,
            members: self.members,
            member_order: self.member_order,
            ctor_signatures,
            top_level_vals: self.top_level_vals,
            enum_variants: self.enum_variants,
            type_constraints,
            interface_fqns,
            class_fqns,
            extensible_class_fqns,
            direct_subtypes,
            supertypes,
            class_ctor_params: self.class_ctor_params,
            super_ctor_delegations: self.super_ctor_delegations,
            type_infos,
            files,
            elements: crate::hir::element::ElementTable::default(),
            lang_items,
        };
        // 声明层 element 表：从上方散表装配（确定性排序）。
        hir.elements = crate::hir::element::assemble(&hir);
        hir
    }
}

/// 在 freeze 边界构建 `HashMap<TypeId, TypeInfo>`。
///
/// 遍历 `index.categories_iter()` 给出的所有 nominal 类型，按类别构造对应的
/// [`crate::hir::type_info::TypeInfo`] 变体；并补充 primitive / tuple / function 条目
/// （从 `TypeStore` 现有内建类型派生）。键为该类型声明对应的 `TypeId`
/// （ref/value nominal 由 `NominalCategory::is_reference` 决定）。
///
/// 设计说明：
/// - 字段 / 方法顺序沿用旧字段（`member_order` / `member_fun_order`）以保证确定性。
/// - 超类型按 `index.category` 拆分 base_class（Class）vs direct_implements（Interface）/
///   direct_extends（Interface）。
/// - `is_final` = 不在 `extensible_class_fqns`（即非 open/abstract）。
#[allow(clippy::too_many_arguments)]
fn build_type_infos(
    store: &mut TypeStore,
    index: &Index,
    interner: &Interner,
    member_funs: &HashMap<Symbol, HashMap<Symbol, Vec<crate::hir::TypedSignature>>>,
    member_fun_order: &HashMap<Symbol, Vec<Symbol>>,
    members: &HashMap<Symbol, HashMap<Symbol, TypeId>>,
    member_order: &HashMap<Symbol, Vec<Symbol>>,
    ctor_signatures: &HashMap<Symbol, Vec<crate::hir::TypedSignature>>,
    enum_variants: &HashMap<Symbol, Vec<Symbol>>,
    extensible_class_fqns: &HashSet<Symbol>,
    class_ctor_params: &HashMap<Symbol, Vec<crate::hir::ClassCtorParamInfo>>,
    super_ctor_delegations: &HashMap<Symbol, crate::hir::SuperCtorDelegation>,
) -> HashMap<TypeId, crate::hir::type_info::TypeInfo> {
    use crate::hir::type_info::{PrimitiveKind, PrimitiveTypeInfo, TypeInfo};
    use crate::resolve::symbol::NominalCategory;

    let mut out: HashMap<TypeId, TypeInfo> = HashMap::new();

    // 按 Symbol（intern 序）排序后遍历：nominal TypeId 的铸造序必须确定
    //（PLAN.md C7——HashMap 迭代序不得进入 TypeStore 分配序 / archive 字节流）。
    let mut categories: Vec<(scoop2_base::Symbol, crate::resolve::symbol::NominalCategory)> =
        index.categories_iter().collect();
    categories.sort_by_key(|(fqn, _)| *fqn);

    for (fqn, category) in categories {
        let is_ref = category.is_reference();
        let ty_id = nominal_type_id(store, fqn, is_ref);
        let info = match category {
            NominalCategory::Class | NominalCategory::Object => {
                let (base_class, direct_implements, _) = split_supertypes(store, index, fqn);
                TypeInfo::Class(build_class_info(
                    store,
                    index,
                    interner,
                    fqn,
                    base_class,
                    direct_implements,
                    member_funs,
                    member_fun_order,
                    members,
                    member_order,
                    ctor_signatures,
                    class_ctor_params,
                    super_ctor_delegations,
                    !extensible_class_fqns.contains(&fqn),
                ))
            }
            NominalCategory::Interface => {
                let (_, _, direct_extends) = split_supertypes(store, index, fqn);
                TypeInfo::Interface(build_interface_info(
                    store,
                    index,
                    interner,
                    fqn,
                    direct_extends,
                    member_funs,
                    member_fun_order,
                ))
            }
            NominalCategory::Struct => {
                let (_, direct_implements, _) = split_supertypes(store, index, fqn);
                TypeInfo::Struct(build_struct_info(
                    store,
                    index,
                    interner,
                    fqn,
                    direct_implements,
                    member_funs,
                    member_fun_order,
                    members,
                    member_order,
                ))
            }
            NominalCategory::Enum => {
                let (_, direct_implements, _) = split_supertypes(store, index, fqn);
                let variants =
                    collect_enum_variants(interner, fqn, enum_variants, members, member_order);
                TypeInfo::Enum(build_enum_info(
                    store,
                    index,
                    interner,
                    fqn,
                    variants,
                    direct_implements,
                    member_funs,
                    member_fun_order,
                ))
            }
            NominalCategory::Effect => TypeInfo::Effect,
        };
        out.insert(ty_id, info);
    }

    // Primitive 条目：从 TypeStore 现有内建值类型派生。
    // 这些类型在 typecheck 期间经 `builtin()`/构造器 intern，故存在于 store。
    for (ty, kind) in [
        (store.unit(), PrimitiveKind::Unit),
        (store.bool(), PrimitiveKind::Bool),
        (store.char(), PrimitiveKind::Char),
        (store.int(), PrimitiveKind::Int),
        (store.uint(), PrimitiveKind::UInt),
        (store.int_n(8), PrimitiveKind::Int8),
        (store.int_n(16), PrimitiveKind::Int16),
        (store.int_n(32), PrimitiveKind::Int32),
        (store.int_n(64), PrimitiveKind::Int64),
        (store.uint_n(8), PrimitiveKind::UInt8),
        (store.uint_n(16), PrimitiveKind::UInt16),
        (store.uint_n(32), PrimitiveKind::UInt32),
        (store.uint_n(64), PrimitiveKind::UInt64),
        (store.float32(), PrimitiveKind::Float32),
        (store.float64(), PrimitiveKind::Float64),
    ] {
        out.entry(ty)
            .or_insert(TypeInfo::Primitive(PrimitiveTypeInfo {
                kind,
                direct_implements: Vec::new(),
            }));
    }

    // Tuple / Function：这两类无具名声明，且 store 内具体元组/函数实例可能很多
    // （每次签名不同 arity/params 都 intern 新类型），无法在 freeze 边界枚举全部。
    // 故此处不预填——HOF / 元组查询改由消费侧按需构造 TypeInfo（后续步骤）。

    out
}

/// 声明态 nominal 的 `TypeId`：无类型实参（args 为空），按 `is_ref` 选 ref/value。
fn nominal_type_id(store: &mut TypeStore, fqn: Symbol, is_ref: bool) -> TypeId {
    use crate::ty::NominalType;
    let n = NominalType {
        fqn,
        args: vec![],
        eff: None,
    };
    if is_ref {
        store.ref_nominal(n)
    } else {
        store.value_nominal(n)
    }
}

/// 超类型按类别拆分：返回 `(base_class, direct_implements, direct_extends)`。
///
/// - `base_class` = supertypes 中为 Class/Object 的第一个（单继承）；
/// - `direct_implements` = 接口超类型（非传递）；
/// - `direct_extends` = 接口超类型（仅对 interface 自身有意义，与 implements 同源）。
fn split_supertypes(
    store: &mut TypeStore,
    index: &Index,
    fqn: Symbol,
) -> (Option<TypeId>, Vec<TypeId>, Vec<TypeId>) {
    use crate::resolve::symbol::NominalCategory;
    let mut base_class: Option<TypeId> = None;
    let mut direct_implements: Vec<TypeId> = Vec::new();
    let mut direct_extends: Vec<TypeId> = Vec::new();
    for &sup in index.supertypes_of(fqn) {
        let is_ref = index.category(sup).is_some_and(|c| c.is_reference());
        let sup_id = nominal_type_id(store, sup, is_ref);
        match index.category(sup) {
            Some(NominalCategory::Class | NominalCategory::Object) => {
                if base_class.is_none() {
                    base_class = Some(sup_id);
                }
            }
            Some(NominalCategory::Interface) => {
                direct_implements.push(sup_id);
                direct_extends.push(sup_id);
            }
            _ => {}
        }
    }
    (base_class, direct_implements, direct_extends)
}

/// 把某 FQN 的有序字段列表收集为 `(name, ty)`，遵循 `member_order`（缺失时按名排序）。
fn ordered_fields(
    interner: &Interner,
    members: &HashMap<Symbol, HashMap<Symbol, TypeId>>,
    member_order: &HashMap<Symbol, Vec<Symbol>>,
    fqn: Symbol,
) -> Vec<(Symbol, TypeId)> {
    let Some(ms) = members.get(&fqn) else {
        return Vec::new();
    };
    match member_order.get(&fqn) {
        Some(order) => order
            .iter()
            .filter_map(|&name| ms.get(&name).map(|&ty| (name, ty)))
            .collect(),
        None => {
            let mut v: Vec<(Symbol, TypeId)> = ms.iter().map(|(&n, &t)| (n, t)).collect();
            v.sort_by(|a, b| interner.resolve(a.0).cmp(interner.resolve(b.0)));
            v
        }
    }
}

/// 把某 FQN 的方法表收集为 `(name, 签名重载集)`，遵循 `member_fun_order`（缺失时按名排序）。
fn ordered_methods(
    interner: &Interner,
    member_funs: &HashMap<Symbol, HashMap<Symbol, Vec<crate::hir::TypedSignature>>>,
    member_fun_order: &HashMap<Symbol, Vec<Symbol>>,
    fqn: Symbol,
) -> Vec<(Symbol, Vec<crate::hir::type_info::Signature>)> {
    let Some(table) = member_funs.get(&fqn) else {
        return Vec::new();
    };
    let names: Vec<Symbol> = match member_fun_order.get(&fqn) {
        Some(order) => order
            .iter()
            .filter(|name| table.contains_key(*name))
            .copied()
            .collect(),
        None => {
            let mut v: Vec<Symbol> = table.keys().copied().collect();
            v.sort_by(|a, b| interner.resolve(*a).cmp(interner.resolve(*b)));
            v
        }
    };
    names
        .into_iter()
        .filter_map(|name| {
            table.get(&name).map(|sigs| {
                (
                    name,
                    sigs.iter()
                        .map(typed_sig_to_type_info_sig)
                        .collect::<Vec<_>>(),
                )
            })
        })
        .collect()
}

/// class 构造器信息（primary_params + secondary + super delegation）。
fn build_class_ctor(
    ctor_signatures: &HashMap<Symbol, Vec<crate::hir::TypedSignature>>,
    class_ctor_params: &HashMap<Symbol, Vec<crate::hir::ClassCtorParamInfo>>,
    super_ctor_delegations: &HashMap<Symbol, crate::hir::SuperCtorDelegation>,
    fqn: Symbol,
) -> crate::hir::type_info::ClassCtor {
    use crate::hir::type_info::{
        ClassCtor as TiClassCtor, ClassCtorParamInfo as TiClassCtorParamInfo,
        SuperCtorDelegation as TiSuperCtorDelegation,
    };
    let primary_params: Vec<TiClassCtorParamInfo> = class_ctor_params
        .get(&fqn)
        .map(|v| {
            v.iter()
                .map(|p| TiClassCtorParamInfo {
                    name: p.name,
                    ty: p.ty,
                    is_property: p.is_property,
                })
                .collect()
        })
        .unwrap_or_default();
    // secondary = ctor_signatures 除首个（primary）外的签名。
    let secondary: Vec<crate::hir::type_info::Signature> = ctor_signatures
        .get(&fqn)
        .map(|v| v.iter().skip(1).map(typed_sig_to_type_info_sig).collect())
        .unwrap_or_default();
    let super_delegation: Option<TiSuperCtorDelegation> =
        super_ctor_delegations
            .get(&fqn)
            .map(|d| TiSuperCtorDelegation {
                super_fqn: d.super_fqn,
                base_index: d.base_index,
                arg_tys: d.arg_tys.clone(),
            });
    TiClassCtor {
        primary_params,
        secondary,
        super_delegation,
    }
}

/// 收集 enum variants（含 payload 字段，登记在 `<enum_fqn>.<variant>` 的 members 下）。
fn collect_enum_variants(
    interner: &Interner,
    fqn: Symbol,
    enum_variants: &HashMap<Symbol, Vec<Symbol>>,
    members: &HashMap<Symbol, HashMap<Symbol, TypeId>>,
    member_order: &HashMap<Symbol, Vec<Symbol>>,
) -> Vec<crate::hir::type_info::EnumVariantInfo> {
    use crate::hir::type_info::EnumVariantInfo;
    enum_variants
        .get(&fqn)
        .map(|vs| {
            vs.iter()
                .map(|&vname| {
                    let variant_fqn_text =
                        format!("{}.{}", interner.resolve(fqn), interner.resolve(vname));
                    let fields = interner
                        .get(&variant_fqn_text)
                        .map(|vfqn| ordered_fields(interner, members, member_order, vfqn))
                        .unwrap_or_default();
                    EnumVariantInfo {
                        name: vname,
                        fields,
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 取一个 nominal 类型声明的类型参数列表（占位，见下方说明）。
///
/// `TypeParamId` / `bound` 无法在 freeze 边界从 `Index` 恢复（Index 不存运行时 id），
/// 故此处返回空列表占位——后续若需完整 type_param 元数据，应在 typecheck 收集阶段
/// 把 (FQN → Vec<TypeParamId>) 一并快照进 `TypedHir`。
fn type_param_decls_for(
    _store: &mut TypeStore,
    _index: &Index,
    _interner: &Interner,
    _fqn: Symbol,
) -> Vec<crate::hir::type_info::TypeParamDecl> {
    // TODO: 后续从 typecheck 收集的 type_constraints / 专门的 type-param 快照表恢复。
    Vec::new()
}

#[allow(clippy::too_many_arguments)]
fn build_class_info(
    store: &mut TypeStore,
    index: &Index,
    interner: &Interner,
    fqn: Symbol,
    base_class: Option<TypeId>,
    direct_implements: Vec<TypeId>,
    member_funs: &HashMap<Symbol, HashMap<Symbol, Vec<crate::hir::TypedSignature>>>,
    member_fun_order: &HashMap<Symbol, Vec<Symbol>>,
    members: &HashMap<Symbol, HashMap<Symbol, TypeId>>,
    member_order: &HashMap<Symbol, Vec<Symbol>>,
    ctor_signatures: &HashMap<Symbol, Vec<crate::hir::TypedSignature>>,
    class_ctor_params: &HashMap<Symbol, Vec<crate::hir::ClassCtorParamInfo>>,
    super_ctor_delegations: &HashMap<Symbol, crate::hir::SuperCtorDelegation>,
    is_final: bool,
) -> crate::hir::type_info::ClassTypeInfo {
    use crate::hir::type_info::ClassTypeInfo;
    ClassTypeInfo {
        type_params: type_param_decls_for(store, index, interner, fqn),
        fields: ordered_fields(interner, members, member_order, fqn),
        methods: ordered_methods(interner, member_funs, member_fun_order, fqn),
        base_class,
        direct_implements,
        ctor: build_class_ctor(
            ctor_signatures,
            class_ctor_params,
            super_ctor_delegations,
            fqn,
        ),
        is_final,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_struct_info(
    store: &mut TypeStore,
    index: &Index,
    interner: &Interner,
    fqn: Symbol,
    direct_implements: Vec<TypeId>,
    member_funs: &HashMap<Symbol, HashMap<Symbol, Vec<crate::hir::TypedSignature>>>,
    member_fun_order: &HashMap<Symbol, Vec<Symbol>>,
    members: &HashMap<Symbol, HashMap<Symbol, TypeId>>,
    member_order: &HashMap<Symbol, Vec<Symbol>>,
) -> crate::hir::type_info::StructTypeInfo {
    use crate::hir::type_info::StructTypeInfo;
    StructTypeInfo {
        type_params: type_param_decls_for(store, index, interner, fqn),
        fields: ordered_fields(interner, members, member_order, fqn),
        methods: ordered_methods(interner, member_funs, member_fun_order, fqn),
        direct_implements,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_interface_info(
    store: &mut TypeStore,
    index: &Index,
    interner: &Interner,
    fqn: Symbol,
    direct_extends: Vec<TypeId>,
    member_funs: &HashMap<Symbol, HashMap<Symbol, Vec<crate::hir::TypedSignature>>>,
    member_fun_order: &HashMap<Symbol, Vec<Symbol>>,
) -> crate::hir::type_info::InterfaceTypeInfo {
    use crate::hir::type_info::InterfaceTypeInfo;
    InterfaceTypeInfo {
        type_params: type_param_decls_for(store, index, interner, fqn),
        methods: ordered_methods(interner, member_funs, member_fun_order, fqn),
        direct_extends,
    }
}

#[allow(clippy::too_many_arguments)]
fn build_enum_info(
    store: &mut TypeStore,
    index: &Index,
    interner: &Interner,
    fqn: Symbol,
    variants: Vec<crate::hir::type_info::EnumVariantInfo>,
    direct_implements: Vec<TypeId>,
    member_funs: &HashMap<Symbol, HashMap<Symbol, Vec<crate::hir::TypedSignature>>>,
    member_fun_order: &HashMap<Symbol, Vec<Symbol>>,
) -> crate::hir::type_info::EnumTypeInfo {
    use crate::hir::type_info::EnumTypeInfo;
    EnumTypeInfo {
        type_params: type_param_decls_for(store, index, interner, fqn),
        variants,
        methods: ordered_methods(interner, member_funs, member_fun_order, fqn),
        direct_implements,
    }
}

/// 把 freeze 后的 [`crate::hir::TypedSignature`] 转换为
/// [`crate::hir::type_info::Signature`]（两者字段同构，仅类型不同）。
///
/// 用于在 `into_typed_hir` 末尾把已转换的签名重打包进 [`crate::hir::type_info::TypeInfo`]。
fn typed_sig_to_type_info_sig(ts: &crate::hir::TypedSignature) -> crate::hir::type_info::Signature {
    crate::hir::type_info::Signature {
        param_types: ts.param_types.clone(),
        return_ty: ts.return_ty,
        type_param_count: ts.type_param_count,
        param_names: ts.param_names.clone(),
        has_defaults: ts.has_defaults.clone(),
        default_exprs: ts.default_exprs.clone(),
        effect_row: ts.effect_row.clone(),
        has_vararg: ts.has_vararg,
        decl_span: ts.decl_span,
        decl_file: ts.decl_file,
    }
}

/// 把签名的 effect 行 AST（`Option<EffectRowExpr>`）解析为规范化的 [`EffectRow`]。
///
/// 与 [`TypeLowering::lower_effect_row`] 同语义（宽容降级，不报 unresolved），
/// 但作为独立函数可在 `into_typed_hir` 末尾对每条签名快照调用，避免在签名构造点
/// 散落 effect 行解析。effect 行参数（`<eff E>`）无声明上下文，按短名保留为 Param。
fn resolve_signature_effect_row(
    store: &mut TypeStore,
    index: &Index,
    interner: &Interner,
    effect: Option<&crate::syntax::ast::EffectRowExpr>,
    eff_param_id: Option<crate::ty::TypeParamId>,
) -> EffectRow {
    use crate::resolve::symbol::NominalCategory;
    use crate::ty::NominalType;
    let Some(eff) = effect else {
        return EffectRow::pure();
    };
    let mut terms: Vec<TypeId> = Vec::new();
    let mut tail = crate::ty::EffectTail::Empty;
    for term in &eff.terms {
        let Some(last) = term.path.segments.last() else {
            continue;
        };
        let last_name = interner.resolve(last.symbol);
        // `Pure` 项不计入行。
        if last_name == "Pure" {
            continue;
        }
        // 单段名：可能是 eff 行参数（`<eff E>` 中的 E）→ 写入 tail = Var(id)，
        // 而非伪装成单个 term（E 代表一整组抽象 effect）。
        if term.path.segments.len() == 1
            && let Some(id) = eff_param_id
        {
            tail = crate::ty::EffectTail::Var(id);
            continue;
        }
        // 多段名：拼 FQN，查 index。
        let fqn_text = term
            .path
            .segments
            .iter()
            .map(|s| interner.resolve(s.symbol).to_string())
            .collect::<Vec<_>>()
            .join(".");
        if let Some(fqn) = interner.get(&fqn_text)
            && index.category(fqn).is_some()
        {
            let is_ref = matches!(
                index.category(fqn),
                Some(NominalCategory::Class | NominalCategory::Interface | NominalCategory::Effect)
            );
            let nominal = NominalType {
                fqn,
                args: vec![],
                eff: None,
            };
            let ty = if is_ref {
                store.ref_nominal(nominal)
            } else {
                store.value_nominal(nominal)
            };
            terms.push(ty);
        }
        // 未解析 → 跳过（宽容）。
    }
    EffectRow::from_terms_with_tail(terms, tail)
}

/// 把文件的**顶层函数**（非扩展、非成员）签名降级并登记进 `env.signatures`。
/// 成员函数 / 构造器 / 扩展函数的签名在成员调用里程碑补齐。
pub fn register_top_level_signatures(
    env: &mut TypeEnv,
    file: &File,
    file_id: FileId,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    for item in &file.items {
        let ItemKind::Fun(d) = &item.kind else {
            continue;
        };
        // 扩展函数：注册到 receiver 类型的 member_signatures（M3 扩展方法调用支持）。
        if let Some(receiver_ref) = &d.receiver {
            // 降级 receiver 类型 → FQN。
            let recv_fqn = {
                let tp_map = build_tp_map(env, d.type_params.as_ref());
                let mut lower =
                    TypeLowering::new(env, imports, tp_map, package_prefix.to_string(), diags);
                let rt = lower.lower(receiver_ref);
                let kind = env.store.kind(rt);
                crate::typecheck::expr::nominal_fqn_of(kind)
                    .or_else(|| crate::typecheck::expr::scalar_fqn(kind, env.interner))
            };
            if let Some(recv_fqn) = recv_fqn {
                let tp_map = build_tp_map(env, d.type_params.as_ref());
                let unit_ty = env.store.unit();
                // 收集 effect 行参数（`<eff E = Pure>` 中的 E）：name → id。
                let eff_params = eff_param_map(d.type_params.as_ref(), &tp_map);
                let eff_param_id = eff_params.values().next().copied();
                let (params, tpb) = {
                    let mut lower =
                        TypeLowering::new(env, imports, tp_map, package_prefix.to_string(), diags);
                    lower.set_eff_params(eff_params.clone());
                    let params: Vec<TypeId> = d
                        .params
                        .iter()
                        .map(|p| match &p.ty {
                            Some(t) => lower.lower(t),
                            None => unit_ty,
                        })
                        .collect();
                    let tpb = lower_type_param_bounds(d.type_params.as_ref(), &mut lower);
                    (params, tpb)
                };
                let return_ty = match &d.return_ty {
                    Some(t) => {
                        let tp_map2 = build_tp_map(env, d.type_params.as_ref());
                        let mut lower = TypeLowering::new(
                            env,
                            imports,
                            tp_map2,
                            package_prefix.to_string(),
                            diags,
                        );
                        lower.set_eff_params(eff_params);
                        lower.lower(t)
                    }
                    None => unit_ty,
                };
                let is_new_fun = !env
                    .member_signatures
                    .get(&recv_fqn)
                    .is_some_and(|m| m.contains_key(&d.name.symbol));
                env.member_signatures
                    .entry(recv_fqn)
                    .or_default()
                    .entry(d.name.symbol)
                    .or_default()
                    .push(Signature {
                        param_names: d.params.iter().map(|p| p.name.symbol).collect(),
                        has_defaults: d.params.iter().map(|p| p.default.is_some()).collect(),
                        default_exprs: d.params.iter().map(|p| p.default.clone()).collect(),
                        has_vararg: d.params.iter().any(|p| p.is_vararg),
                        params,
                        return_ty,
                        type_param_count: d
                            .type_params
                            .as_ref()
                            .map(|tp| tp.params.len())
                            .unwrap_or(0),
                        type_param_bounds: tpb,
                        modifiers: crate::resolve::symbol::ModifierSet::from_modifiers(
                            &d.modifiers,
                        ),
                        effect: d.effect.clone(),
                        eff_param_id,
                        has_body: d.body.is_some(),
                        decl_span: d.name.span,
                        decl_file: file_id,
                    });
                if is_new_fun {
                    env.member_fun_order
                        .entry(recv_fqn)
                        .or_default()
                        .push(d.name.symbol);
                }
            }
            continue;
        }
        let name_text = env.interner.resolve(d.name.symbol);
        let fqn_text = if package_prefix.is_empty() {
            name_text.to_string()
        } else {
            format!("{package_prefix}.{name_text}")
        };
        let Some(fqn) = env.interner.get(&fqn_text) else {
            continue;
        };
        // 类型参数映射（用于降低签名中的类型参数引用）。
        let tp_map = build_tp_map(env, d.type_params.as_ref());
        let tpc = tp_map.len();
        let unit_ty = env.store.unit();
        let (prb, pvb) = collect_param_kind_bounds(d.type_params.as_ref(), d.where_clause.as_ref());
        // 收集 effect 行参数（`<eff E = Pure>` 中的 E）：name → id。
        let eff_params = eff_param_map(d.type_params.as_ref(), &tp_map);
        let eff_param_id = eff_params.values().next().copied();
        let sig = {
            let mut lower = TypeLowering::with_bounds(
                env,
                imports,
                tp_map,
                package_prefix.to_string(),
                diags,
                prb,
                pvb,
            );
            lower.set_eff_params(eff_params);
            let params: Vec<TypeId> = d
                .params
                .iter()
                .map(|p| match &p.ty {
                    Some(t) => lower.lower(t),
                    None => unit_ty,
                })
                .collect();
            let return_ty = match &d.return_ty {
                Some(t) => lower.lower(t),
                None => unit_ty,
            };
            Signature {
                param_names: d.params.iter().map(|p| p.name.symbol).collect(),
                has_defaults: d.params.iter().map(|p| p.default.is_some()).collect(),
                default_exprs: d.params.iter().map(|p| p.default.clone()).collect(),
                has_vararg: d.params.iter().any(|p| p.is_vararg),
                params,
                return_ty,
                type_param_count: tpc,
                type_param_bounds: lower_type_param_bounds(d.type_params.as_ref(), &mut lower),
                modifiers: crate::resolve::symbol::ModifierSet::from_modifiers(&d.modifiers),
                effect: d.effect.clone(),
                eff_param_id,
                has_body: d.body.is_some(),
                decl_span: d.name.span,
                decl_file: file_id,
            }
        };
        env.signatures.entry(fqn).or_default().push(sig);
        env.fun_attrs.insert(
            fqn,
            FunAttrs {
                is_nogc: has_annotation(&d.annotations, "NoGC", env.interner),
                is_native_extern: is_native_c_extern(&d.annotations, env.interner),
            },
        );
    }
}

/// 注解路径末段文本匹配。
fn has_annotation(
    anns: &[crate::syntax::ast::AnnotationUse],
    name: &str,
    interner: &Interner,
) -> bool {
    anns.iter().any(|a| {
        a.path
            .segments
            .last()
            .is_some_and(|s| interner.resolve(s.symbol) == name)
    })
}

/// `@Extern` 且 ABI 为缺省或 `"c"`（native leaf）。
fn is_native_c_extern(anns: &[crate::syntax::ast::AnnotationUse], interner: &Interner) -> bool {
    use crate::syntax::ast::ExprKind;
    let Some(ext) = anns.iter().find(|a| {
        a.path
            .segments
            .last()
            .is_some_and(|s| interner.resolve(s.symbol) == "Extern")
    }) else {
        return false;
    };
    // 若给出 `abi=`，必须是 "c"。
    for arg in &ext.args {
        if arg
            .name
            .as_ref()
            .is_some_and(|n| interner.resolve(n.symbol) == "abi")
        {
            if let ExprKind::StringLit(s) = &arg.value.kind {
                return s.value.trim().eq_ignore_ascii_case("c");
            }
            return false;
        }
    }
    true
}

/// 把文件的类型 / object 的**属性成员**（含主构造 param-property）类型降级并登记进
/// `env.members`。成员函数 / variant 不在此（它们不是值成员读取的目标）。
pub fn register_members(
    env: &mut TypeEnv,
    file: &File,
    file_id: FileId,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    for item in &file.items {
        match &item.kind {
            ItemKind::Type(d) => {
                let owner = fqn_of(env, package_prefix, d.name.symbol);
                // 主构造 param-property（`class C(val x: T)`）。
                if let Some(ctor) = &d.primary_ctor {
                    for cp in &ctor.params {
                        if let Some(kind) = &cp.property
                            && let Some(ty) = &cp.ty
                        {
                            lower_and_store_member(
                                env,
                                owner,
                                cp.name.symbol,
                                ty,
                                imports,
                                package_prefix,
                                d.type_params.as_ref(),
                                diags,
                            );
                            // `val` param-property 登记为不可变。
                            if *kind == crate::syntax::ast::ValKind::Val {
                                env.immutable_members
                                    .entry(owner)
                                    .or_default()
                                    .insert(cp.name.symbol);
                            }
                        }
                    }
                }
                if let Some(body) = &d.body {
                    register_body_members(
                        env,
                        owner,
                        &body.members,
                        d.type_params.as_ref(),
                        imports,
                        package_prefix,
                        diags,
                        file_id,
                    );
                }
            }
            ItemKind::Object(d) => {
                if let Some(name) = &d.name
                    && let Some(body) = &d.body
                {
                    let owner = fqn_of(env, package_prefix, name.symbol);
                    register_body_members(
                        env,
                        owner,
                        &body.members,
                        None,
                        imports,
                        package_prefix,
                        diags,
                        file_id,
                    );
                }
            }
            _ => {}
        }
    }
}

/// 登记类型体成员：属性 → 成员类型；嵌套类型 / object 递归；companion 成员挂到 owner。
#[allow(clippy::too_many_arguments)]
fn register_body_members(
    env: &mut TypeEnv,
    owner: Symbol,
    members: &[TypeMember],
    type_params: Option<&TypeParamList>,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
    file_id: FileId,
) {
    for m in members {
        match &m.kind {
            TypeMemberKind::Property(d) => {
                if let Some(ty) = &d.ty {
                    lower_and_store_member(
                        env,
                        owner,
                        d.name.symbol,
                        ty,
                        imports,
                        package_prefix,
                        type_params,
                        diags,
                    );
                }
                // `val` 属性登记为不可变（赋值目标可变性检查）。
                if d.kind == crate::syntax::ast::ValKind::Val {
                    env.immutable_members
                        .entry(owner)
                        .or_default()
                        .insert(d.name.symbol);
                }
                // 无类型标注的属性（推断）→ M2 暂不登记（需 init 推断，后续里程碑）。
            }
            TypeMemberKind::Object(d) => {
                if d.companion {
                    if let Some(b) = &d.body {
                        register_body_members(
                            env,
                            owner,
                            &b.members,
                            None,
                            imports,
                            package_prefix,
                            diags,
                            file_id,
                        );
                    }
                } else if let Some(name) = &d.name
                    && let Some(b) = &d.body
                {
                    let nested = fqn_under(env, owner, name.symbol);
                    register_body_members(
                        env,
                        nested,
                        &b.members,
                        None,
                        imports,
                        package_prefix,
                        diags,
                        file_id,
                    );
                }
            }
            TypeMemberKind::Type(d) => {
                if let Some(b) = &d.body {
                    let nested = fqn_under(env, owner, d.name.symbol);
                    register_body_members(
                        env,
                        nested,
                        &b.members,
                        d.type_params.as_ref(),
                        imports,
                        package_prefix,
                        diags,
                        file_id,
                    );
                }
            }
            TypeMemberKind::Fun(d) => {
                // 注册外层类型 + 成员函数自身的类型参数（各自稳定 id）。
                let mut tp_map = build_tp_map(env, type_params);
                if d.type_params.is_some() {
                    let m = build_tp_map(env, d.type_params.as_ref());
                    tp_map.extend(m);
                }
                // effect 行参数（优先成员函数自身声明，其次外层类型）。
                let eff_params = eff_param_map(d.type_params.as_ref(), &tp_map);
                let eff_params = if eff_params.is_empty() {
                    eff_param_map(type_params, &tp_map)
                } else {
                    eff_params
                };
                let unit_ty = env.store.unit();
                let sig = {
                    let mut lower =
                        TypeLowering::new(env, imports, tp_map, package_prefix.to_string(), diags);
                    lower.set_eff_params(eff_params.clone());
                    let params: Vec<TypeId> = d
                        .params
                        .iter()
                        .map(|p| match &p.ty {
                            Some(t) => lower.lower(t),
                            None => unit_ty,
                        })
                        .collect();
                    let return_ty = match &d.return_ty {
                        Some(t) => lower.lower(t),
                        None => unit_ty,
                    };
                    Signature {
                        param_names: d.params.iter().map(|p| p.name.symbol).collect(),
                        has_defaults: d.params.iter().map(|p| p.default.is_some()).collect(),
                        default_exprs: d.params.iter().map(|p| p.default.clone()).collect(),
                        has_vararg: d.params.iter().any(|p| p.is_vararg),
                        params,
                        return_ty,
                        type_param_count: d
                            .type_params
                            .as_ref()
                            .map(|tp| tp.params.len())
                            .unwrap_or(0),
                        type_param_bounds: lower_type_param_bounds(
                            d.type_params.as_ref(),
                            &mut lower,
                        ),
                        modifiers: crate::resolve::symbol::ModifierSet::from_modifiers(
                            &d.modifiers,
                        ),
                        effect: d.effect.clone(),
                        eff_param_id: eff_params.values().next().copied(),
                        has_body: d.body.is_some(),
                        decl_span: d.name.span,
                        decl_file: file_id,
                    }
                };
                let is_new_fun = !env
                    .member_signatures
                    .get(&owner)
                    .is_some_and(|m| m.contains_key(&d.name.symbol));
                env.member_signatures
                    .entry(owner)
                    .or_default()
                    .entry(d.name.symbol)
                    .or_default()
                    .push(sig);
                if is_new_fun {
                    env.member_fun_order
                        .entry(owner)
                        .or_default()
                        .push(d.name.symbol);
                }
            }
            TypeMemberKind::EnumVariant(ev) => {
                // 把 variant 的字段类型注册到 `<enum_fqn>.<variant_name>` 的 members。
                let variant_fqn_text = format!(
                    "{}.{}",
                    env.interner.resolve(owner),
                    env.interner.resolve(ev.name.symbol)
                );
                if let Some(variant_fqn) = env.interner.get(&variant_fqn_text) {
                    let tp_map = build_tp_map(env, type_params);
                    for field in &ev.fields {
                        let mut lower = TypeLowering::new(
                            env,
                            imports,
                            tp_map.clone(),
                            package_prefix.to_string(),
                            diags,
                        );
                        let ft = lower.lower(&field.ty);
                        let is_new = !env
                            .members
                            .get(&variant_fqn)
                            .is_some_and(|m| m.contains_key(&field.name.symbol));
                        env.members
                            .entry(variant_fqn)
                            .or_default()
                            .insert(field.name.symbol, ft);
                        if is_new {
                            env.member_order
                                .entry(variant_fqn)
                                .or_default()
                                .push(field.name.symbol);
                        }
                    }
                }
            }
            TypeMemberKind::InitBlock(_) | TypeMemberKind::SecondaryCtor(_) => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_and_store_member(
    env: &mut TypeEnv,
    owner: Symbol,
    name: Symbol,
    ty: &crate::syntax::ast::TypeRef,
    imports: &ImportTable,
    package_prefix: &str,
    type_params: Option<&TypeParamList>,
    diags: &mut DiagnosticSink,
) {
    let tp_map = build_tp_map(env, type_params);
    let lowered = {
        let mut lower = TypeLowering::new(env, imports, tp_map, package_prefix.to_string(), diags);
        lower.lower(ty)
    };
    let is_new = !env
        .members
        .get(&owner)
        .is_some_and(|m| m.contains_key(&name));
    env.members.entry(owner).or_default().insert(name, lowered);
    // 声明序侧表：同名成员重复登记（如主构造 param-property 与体内同名声明）
    // 只保留首次出现的位置。
    if is_new {
        env.member_order.entry(owner).or_default().push(name);
    }
}

/// 把 AST 类型参数列表注册为 `TypeParamDecl`（每个参数分配稳定的
/// `TypeParamId = NodeId.as_u32()`，按声明位全局唯一），返回 name → id 映射，
/// 供 [`TypeLowering`] 按名字查到 id。
///
/// 幂等：同一 AST `TypeParam` 的 `id: NodeId` 不变，故多次调用注册同一 id，
/// 不会重复分配——这保证了同声明内 `build_tp_map` 被多次调用（参数/返回值/bound
/// 各一次）时 id 一致。
pub(super) fn build_tp_map(
    env: &mut TypeEnv,
    tpl: Option<&TypeParamList>,
) -> HashMap<Symbol, crate::ty::TypeParamId> {
    let mut map = HashMap::new();
    if let Some(tpl) = tpl {
        for p in &tpl.params {
            let id = crate::ty::TypeParamId(p.id.as_u32());
            env.store.mint_param(crate::ty::TypeParamDecl {
                id,
                name: p.name.symbol,
                file: FileId(0),
                span: p.name.span,
                variance: p.variance.map(ast_variance_to_ty),
                bound: None, // bound 在 lower_type_param_bounds 阶段单独填入
                kind: crate::ty::TypeParamKind::Type,
            });
            map.insert(p.name.symbol, id);
        }
        // effect 行参数（<eff E>）也注册，kind = Effect。
        if let Some(er) = &tpl.effect_row {
            let id = crate::ty::TypeParamId(er.id.as_u32());
            env.store.mint_param(crate::ty::TypeParamDecl {
                id,
                name: er.name.symbol,
                file: FileId(0),
                span: er.name.span,
                variance: None,
                bound: None,
                kind: crate::ty::TypeParamKind::Effect,
            });
            map.insert(er.name.symbol, id);
        }
    }
    map
}

/// AST 的 [`crate::syntax::ast::Variance`] → ty 模块的 [`crate::ty::Variance`]。
fn ast_variance_to_ty(v: crate::syntax::ast::Variance) -> crate::ty::Variance {
    match v {
        crate::syntax::ast::Variance::In => crate::ty::Variance::In,
        crate::syntax::ast::Variance::Out => crate::ty::Variance::Out,
    }
}

/// 从已注册的 `tp_map`（name → id，含普通参数 + effect 行参数）中抽取 effect
/// 行参数（`<eff E>`）的 name → id 子映射，供 [`TypeLowering::set_eff_params`]。
fn eff_param_map(
    tpl: Option<&TypeParamList>,
    tp_map: &HashMap<Symbol, crate::ty::TypeParamId>,
) -> HashMap<Symbol, crate::ty::TypeParamId> {
    let mut out = HashMap::new();
    if let Some(tpl) = tpl
        && let Some(er) = &tpl.effect_row
    {
        if let Some(&id) = tp_map.get(&er.name.symbol) {
            out.insert(er.name.symbol, id);
        }
    }
    out
}

/// 收集类型参数列表与 where 子句中声明了 `ref` / `value` kind bound 的参数名集合。
/// 用于约束 forward 检查：`<U: ref>` 的 U 转发给 `<T: ref>` 函数是合法的。
pub(super) fn collect_param_kind_bounds(
    tpl: Option<&TypeParamList>,
    wc: Option<&crate::syntax::ast::WhereClause>,
) -> (HashSet<Symbol>, HashSet<Symbol>) {
    use crate::syntax::ast::GenericBound;
    let mut ref_bounds = HashSet::new();
    let mut value_bounds = HashSet::new();
    if let Some(tpl) = tpl {
        for p in &tpl.params {
            if let Some(b) = &p.bound {
                match b {
                    GenericBound::Ref(_) => {
                        ref_bounds.insert(p.name.symbol);
                    }
                    GenericBound::Value(_) => {
                        value_bounds.insert(p.name.symbol);
                    }
                    _ => {}
                }
            }
        }
    }
    if let Some(wc) = wc {
        for c in &wc.constraints {
            match &c.bound {
                GenericBound::Ref(_) => {
                    ref_bounds.insert(c.name.symbol);
                }
                GenericBound::Value(_) => {
                    value_bounds.insert(c.name.symbol);
                }
                _ => {}
            }
        }
    }
    (ref_bounds, value_bounds)
}

fn fqn_of(env: &TypeEnv, package_prefix: &str, name: Symbol) -> Symbol {
    let name_text = env.interner.resolve(name);
    let fqn_text = if package_prefix.is_empty() {
        name_text.to_string()
    } else {
        format!("{package_prefix}.{name_text}")
    };
    env.interner.get(&fqn_text).unwrap_or(name)
}

/// 降级类型参数列表中各参数的 `Type` bound（无 bound / ref/value bound → None）。
fn lower_type_param_bounds(
    tpl: Option<&crate::syntax::ast::TypeParamList>,
    lower: &mut TypeLowering,
) -> Vec<Option<TypeId>> {
    use crate::syntax::ast::GenericBound;
    let Some(tpl) = tpl else {
        return Vec::new();
    };
    tpl.params
        .iter()
        .map(|p| match &p.bound {
            Some(GenericBound::Type(t)) => Some(lower.lower(t)),
            _ => None,
        })
        .collect()
}

fn fqn_under(env: &TypeEnv, owner: Symbol, name: Symbol) -> Symbol {
    let owner_text = env.interner.resolve(owner);
    let name_text = env.interner.resolve(name);
    env.interner
        .get(&format!("{owner_text}.{name_text}"))
        .unwrap_or(name)
}

/// 收集 enum 的 variant 名（FQN → [variant_name]），供 when 穷尽性检查。
pub fn register_enum_variants(env: &mut TypeEnv, file: &File, package_prefix: &str) {
    for item in &file.items {
        let ItemKind::Type(d) = &item.kind else {
            continue;
        };
        if !matches!(d.kind, crate::syntax::ast::TypeKind::Enum) {
            continue;
        }
        let fqn = fqn_of(env, package_prefix, d.name.symbol);
        let mut variants: Vec<Symbol> = Vec::new();
        if let Some(body) = &d.body {
            for m in &body.members {
                if let TypeMemberKind::EnumVariant(ev) = &m.kind {
                    variants.push(ev.name.symbol);
                    // 登记 variant payload arity（键 (enum FQN, variant 名)）。
                    env.enum_variant_arities
                        .insert((fqn, ev.name.symbol), ev.fields.len());
                }
            }
        }
        env.enum_variants.insert(fqn, variants);
    }
}

/// 收集顶层 val/var 的类型（简单名 → TypeId），供表达式引用解析。
pub fn register_top_level_vals(
    env: &mut TypeEnv,
    file: &File,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    for item in &file.items {
        let ItemKind::Val(d) = &item.kind else {
            continue;
        };
        let ValBinding::Name(name) = &d.binding else {
            continue;
        };
        if let Some(ty_ref) = &d.ty {
            let ty = {
                let mut lower = TypeLowering::new(
                    env,
                    imports,
                    HashMap::new(),
                    package_prefix.to_string(),
                    diags,
                );
                lower.lower(ty_ref)
            };
            env.top_level_vals.insert(name.symbol, ty);
        }
    }
}

/// 收集类型/函数的 where 约束（FQN → (参数名序列, 约束)），供类型实参满足性检查。
pub fn register_type_constraints(
    env: &mut TypeEnv,
    file: &File,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::{GenericBound, ItemKind, TypeParamList, WhereClause};
    let collect_one = |env: &mut TypeEnv,
                       name: Symbol,
                       type_params: Option<&TypeParamList>,
                       where_clause: Option<&WhereClause>| {
        let owner = fqn_of(env, package_prefix, name);
        let param_names: Vec<Symbol> = type_params
            .map(|tp| tp.params.iter().map(|p| p.name.symbol).collect())
            .unwrap_or_default();
        let mut constraints: Vec<(Symbol, GenericBound)> = Vec::new();
        if let Some(wc) = where_clause {
            for c in &wc.constraints {
                constraints.push((c.name.symbol, c.bound.clone()));
            }
        }
        if let Some(tp) = type_params {
            for p in &tp.params {
                if let Some(b) = &p.bound {
                    constraints.push((p.name.symbol, b.clone()));
                }
            }
        }
        env.type_constraints
            .insert(owner, (param_names, constraints));
        // 记录该类型/函数是否声明了 eff 形参。
        if type_params.is_some_and(|tp| tp.effect_row.is_some()) {
            env.eff_param_types.insert(owner);
        }
    };
    for item in &file.items {
        match &item.kind {
            ItemKind::Type(d) => {
                collect_one(
                    env,
                    d.name.symbol,
                    d.type_params.as_ref(),
                    d.where_clause.as_ref(),
                );
                // 注册直接超类型实例化（FQN + 类型实参）。
                let owner = fqn_of(env, package_prefix, d.name.symbol);
                let tp_map = build_tp_map(env, d.type_params.as_ref());
                let mut sup_instances: Vec<(Symbol, Vec<TypeId>)> = Vec::new();
                for st in &d.supertypes {
                    let mut lower = TypeLowering::new(
                        env,
                        imports,
                        tp_map.clone(),
                        package_prefix.to_string(),
                        diags,
                    );
                    let st_ty = lower.lower(&st.ty);
                    let st_kind = env.store.kind(st_ty);
                    if let Some(st_fqn) = crate::typecheck::expr::nominal_fqn_of(st_kind) {
                        let st_args: Vec<TypeId> = crate::typecheck::expr::nominal_args_of(st_kind)
                            .unwrap_or(&[])
                            .to_vec();
                        sup_instances.push((st_fqn, st_args));
                    }
                }
                env.register_supertype_instances(owner, sup_instances);
            }
            ItemKind::Fun(d) if d.receiver.is_none() => {
                collect_one(
                    env,
                    d.name.symbol,
                    d.type_params.as_ref(),
                    d.where_clause.as_ref(),
                );
            }
            _ => {}
        }
    }
}

/// 收集文件中的 typealias（FQN → RHS + 类型参数名），供 lower 展开；并检测循环别名。
pub fn register_type_aliases(
    env: &mut TypeEnv,
    file: &File,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    use crate::syntax::ast::TypeRefKind;
    let mut declared: Vec<(Symbol, scoop2_base::Span)> = Vec::new();
    for item in &file.items {
        let ItemKind::TypeAlias(d) = &item.kind else {
            continue;
        };
        let owner = fqn_of(env, package_prefix, d.name.symbol);
        let params: Vec<Symbol> = d
            .type_params
            .as_ref()
            .map(|tp| tp.params.iter().map(|p| p.name.symbol).collect())
            .unwrap_or_default();
        env.type_aliases.insert(owner, (d.ty.clone(), params));
        declared.push((owner, d.name.span));
    }
    // 循环别名检测：沿直接别名引用链跟踪，若回到起点或已访问节点则报错。
    let direct_alias_target = |ty: &crate::syntax::ast::TypeRef| -> Option<Symbol> {
        if let TypeRefKind::Path { path, .. } = &ty.kind
            && let Some(last) = path.segments.last()
        {
            let name = env.interner.resolve(last.symbol);
            let candidates = [name.to_string(), format!("{package_prefix}.{name}")];
            candidates.into_iter().find_map(|c| {
                let sym = env.interner.get(&c)?;
                env.type_aliases.contains_key(&sym).then_some(sym)
            })
        } else {
            None
        }
    };
    for (start, span) in &declared {
        let mut visiting = vec![*start];
        let Some((mut current, _)) = env.type_aliases.get(start).cloned() else {
            continue;
        };
        for _ in 0..64 {
            let Some(target) = direct_alias_target(&current) else {
                break;
            };
            if target == *start || visiting.contains(&target) {
                diags.push(super::diagnostics::cyclic_type_alias(*span));
                break;
            }
            visiting.push(target);
            match env.type_aliases.get(&target) {
                Some((t, _)) => current = t.clone(),
                None => break,
            }
        }
    }
}

/// 收集文件中带 `@CLayout` 注解的 struct FQN（native `@Extern` ABI 允许的 nominal 值类型）。
pub fn register_clayout_structs(env: &mut TypeEnv, file: &File, package_prefix: &str) {
    for item in &file.items {
        let ItemKind::Type(d) = &item.kind else {
            continue;
        };
        if d.kind != crate::syntax::ast::TypeKind::Struct {
            continue;
        }
        if d.annotations
            .iter()
            .any(|a| annotation_last_text(a, env.interner) == Some("CLayout"))
        {
            env.clayout_structs
                .insert(fqn_of(env, package_prefix, d.name.symbol));
        }
    }
}

/// 注解路径末段文本（用于 `@CLayout` 等内建注解识别）。
fn annotation_last_text<'i>(
    ann: &crate::syntax::ast::AnnotationUse,
    interner: &'i Interner,
) -> Option<&'i str> {
    ann.path.segments.last().map(|s| interner.resolve(s.symbol))
}

/// 把类型的主构造参数类型降级并登记进 `env.ctors`（用于 `Type(args)` 构造器调用）。
pub fn register_constructors(
    env: &mut TypeEnv,
    file: &File,
    file_id: FileId,
    imports: &ImportTable,
    package_prefix: &str,
    diags: &mut DiagnosticSink,
) {
    for item in &file.items {
        let ItemKind::Type(d) = &item.kind else {
            continue;
        };
        let owner = fqn_of(env, package_prefix, d.name.symbol);
        if let Some(ctor) = &d.primary_ctor {
            let tp_map = build_tp_map(env, d.type_params.as_ref());
            let unit_ty = env.store.unit();
            let params: Vec<TypeId> = ctor
                .params
                .iter()
                .map(|cp| match &cp.ty {
                    Some(t) => {
                        let mut lower = TypeLowering::new(
                            env,
                            imports,
                            tp_map.clone(),
                            package_prefix.to_string(),
                            diags,
                        );
                        lower.lower(t)
                    }
                    None => unit_ty,
                })
                .collect();
            env.ctors.insert(owner, params);
        } else if d.primary_ctor.is_none()
            && let Some(body) = &d.body
        {
            // 无主构造器头（`struct S { val a: T }`）：从 body 的属性字段合成构造参数。
            use crate::syntax::ast::TypeMemberKind;
            let tp_map = build_tp_map(env, d.type_params.as_ref());
            let unit_ty = env.store.unit();
            let mut params: Vec<TypeId> = Vec::new();
            let mut names: Vec<Symbol> = Vec::new();
            let mut defaults: Vec<bool> = Vec::new();
            let mut vararg = false;
            for m in &body.members {
                if let TypeMemberKind::Property(pd) = &m.kind
                    && pd.accessors.is_empty()
                {
                    let pt = pd
                        .ty
                        .as_ref()
                        .map(|t| {
                            let mut lower = TypeLowering::new(
                                env,
                                imports,
                                tp_map.clone(),
                                package_prefix.to_string(),
                                diags,
                            );
                            lower.lower(t)
                        })
                        .unwrap_or(unit_ty);
                    params.push(pt);
                    names.push(pd.name.symbol);
                    defaults.push(pd.init.is_some());
                    // PropertyDecl 无 vararg 标记；body-field 属性构造参数不是 vararg。
                    vararg = false;
                }
            }
            if !params.is_empty() {
                env.ctors.insert(owner, params.clone());
                // 也注册为 ctor_signatures（使 resolve_ctor_overloads 处理命名实参 / vararg）。
                let tp_count = d
                    .type_params
                    .as_ref()
                    .map(|tp| tp.params.len())
                    .unwrap_or(0);
                let tp_bounds = lower_type_param_bounds(
                    d.type_params.as_ref(),
                    &mut TypeLowering::new(env, imports, tp_map, package_prefix.to_string(), diags),
                );
                let n_params = params.len();
                env.ctor_signatures.insert(
                    owner,
                    vec![Signature {
                        params,
                        return_ty: unit_ty,
                        type_param_count: tp_count,
                        type_param_bounds: tp_bounds,
                        param_names: names,
                        has_defaults: defaults,
                        default_exprs: vec![None; n_params],
                        has_vararg: vararg,
                        modifiers: crate::resolve::symbol::ModifierSet::default(),
                        effect: None,
                        eff_param_id: None,
                        has_body: true,
                        decl_span: d.name.span,
                        decl_file: file_id,
                    }],
                );
            }
        }
        // 次构造器签名（M3 构造器重载决议用）。若有次构造器，则连同主构造器一起登记。
        if let Some(body) = &d.body {
            use crate::syntax::ast::TypeMemberKind;
            // 合并类型自身 + 次构造器的类型参数（次构造器可引用类型的类型参数）。
            let type_tp_count = d
                .type_params
                .as_ref()
                .map(|tp| tp.params.len())
                .unwrap_or(0);
            let mut secondary: Vec<Signature> = Vec::new();
            for m in &body.members {
                let TypeMemberKind::SecondaryCtor(c) = &m.kind else {
                    continue;
                };
                let mut tp_map = build_tp_map(env, d.type_params.as_ref());
                tp_map.extend(build_tp_map(env, c.type_params.as_ref()));
                let unit_ty = env.store.unit();
                let (params, tpb) = {
                    let mut lower =
                        TypeLowering::new(env, imports, tp_map, package_prefix.to_string(), diags);
                    let params: Vec<TypeId> = c
                        .params
                        .iter()
                        .map(|p| match &p.ty {
                            Some(t) => lower.lower(t),
                            None => unit_ty,
                        })
                        .collect();
                    // 次构造器自身的类型参数 bound（类型自身的类型参数 bound 由类型负责）。
                    let tpb = lower_type_param_bounds(c.type_params.as_ref(), &mut lower);
                    (params, tpb)
                };
                secondary.push(Signature {
                    param_names: c.params.iter().map(|p| p.name.symbol).collect(),
                    has_defaults: c.params.iter().map(|p| p.default.is_some()).collect(),
                    default_exprs: c.params.iter().map(|p| p.default.clone()).collect(),
                    has_vararg: c.params.iter().any(|p| p.is_vararg),
                    params,
                    return_ty: unit_ty,
                    type_param_count: c
                        .type_params
                        .as_ref()
                        .map(|tp| tp.params.len())
                        .unwrap_or(0),
                    type_param_bounds: tpb,
                    modifiers: crate::resolve::symbol::ModifierSet::from_modifiers(&c.modifiers),
                    effect: None,
                    eff_param_id: None,
                    has_body: true,
                    decl_span: c.span,
                    decl_file: file_id,
                });
            }
            // 主构造器始终注册为候选（即使无次构造器），使 resolve_ctor_overloads 统一处理
            // arity / vararg / 默认值（否则仅主构造器的类走 ctor_params + check_call_args，
            // 无法识别 vararg）。
            if let Some(mut primary_params) = env.ctors.get(&owner).cloned() {
                // 若有主构造器，追加 body-field 属性的类型到主构造器参数（使 param_names 与 params 对齐）。
                // 无主构造器的 struct 的 body-field 已在合成路径中处理，不重复追加。
                if d.primary_ctor.is_some() {
                    let tp_map = build_tp_map(env, d.type_params.as_ref());
                    let unit_ty = env.store.unit();
                    for m in &body.members {
                        if let TypeMemberKind::Property(pd) = &m.kind
                            && pd.accessors.is_empty()
                        {
                            let pt = pd
                                .ty
                                .as_ref()
                                .map(|t| {
                                    let mut lower = TypeLowering::new(
                                        env,
                                        imports,
                                        tp_map.clone(),
                                        package_prefix.to_string(),
                                        diags,
                                    );
                                    lower.lower(t)
                                })
                                .unwrap_or(unit_ty);
                            primary_params.push(pt);
                        }
                    }
                    let _ = tp_map;
                }
                let primary_tp_bounds = {
                    let tp_map = build_tp_map(env, d.type_params.as_ref());
                    let mut lower =
                        TypeLowering::new(env, imports, tp_map, package_prefix.to_string(), diags);
                    lower_type_param_bounds(d.type_params.as_ref(), &mut lower)
                };
                let primary_defaults: Vec<bool> = d
                    .primary_ctor
                    .iter()
                    .flat_map(|pc| pc.params.iter().map(|cp| cp.default.is_some()))
                    .chain(body.members.iter().filter_map(|m| match &m.kind {
                        TypeMemberKind::Property(pd) => Some(pd.init.is_some()),
                        _ => None,
                    }))
                    .collect();
                let primary_names: Vec<Symbol> = d
                    .primary_ctor
                    .iter()
                    .flat_map(|pc| pc.params.iter().map(|cp| cp.name.symbol))
                    .chain(body.members.iter().filter_map(|m| match &m.kind {
                        TypeMemberKind::Property(pd) if pd.accessors.is_empty() => {
                            Some(pd.name.symbol)
                        }
                        _ => None,
                    }))
                    .collect();
                let primary_vararg = d
                    .primary_ctor
                    .iter()
                    .flat_map(|pc| pc.params.iter())
                    .any(|cp| cp.is_vararg);
                // primary_defaults / primary_default_exprs 与 primary_params 等长：
                // body-field 追加的参数无默认值表达式，但标记 has_defaults=true（保持
                // 适用性——body-field 是属性初始化器，调用点不传它们，但不报错）。
                let primary_ctor_n = d
                    .primary_ctor
                    .iter()
                    .flat_map(|pc| pc.params.iter())
                    .count();
                let body_field_count = primary_params.len().saturating_sub(primary_ctor_n);
                let primary_default_exprs = {
                    let mut v: Vec<Option<crate::syntax::ast::Expr>> = d
                        .primary_ctor
                        .iter()
                        .flat_map(|pc| pc.params.iter())
                        .map(|cp| cp.default.clone())
                        .collect();
                    // body-field 参数无默认值表达式（None）。
                    for _ in 0..body_field_count {
                        v.push(None);
                    }
                    v
                };
                let primary_defaults = {
                    let mut v = primary_defaults;
                    // body-field 参数标记为 has_defaults=true（保持适用性，不报缺少参数）。
                    for _ in 0..body_field_count {
                        v.push(true);
                    }
                    v
                };
                let mut all = vec![Signature {
                    param_names: primary_names,
                    has_defaults: primary_defaults,
                    default_exprs: primary_default_exprs,
                    has_vararg: primary_vararg,
                    params: primary_params,
                    return_ty: env.store.unit(),
                    type_param_count: type_tp_count,
                    type_param_bounds: primary_tp_bounds,
                    modifiers: crate::resolve::symbol::ModifierSet::default(),
                    effect: None,
                    eff_param_id: None,
                    has_body: true,
                    decl_span: d
                        .primary_ctor
                        .as_ref()
                        .map(|pc| pc.span)
                        .unwrap_or(d.name.span),
                    decl_file: file_id,
                }];
                all.extend(secondary);
                env.ctor_signatures.insert(owner, all);
            } else if !secondary.is_empty() {
                // 无 primary_ctor 但有 secondary ctor：只收集 secondary
                //（无 primary_ctor 的类仍可通过 secondary ctor 构造）。
                env.ctor_signatures.insert(owner, secondary);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::index::Index;

    #[test]
    fn builtin_scalars_and_qualified_form() {
        let idx = Index::new();
        let it = Interner::new();
        let mut env = TypeEnv::new(&idx, &it);
        let i = env.builtin("Int").unwrap();
        let i2 = env.builtin("scoop.core.Int").unwrap();
        assert_eq!(i, i2, "short and qualified resolve to same TypeId");
        assert!(env.store.is_value(i));
        let s = env.builtin("String").unwrap();
        assert!(env.store.is_reference(s));
        let u = env.builtin("Unit").unwrap();
        assert!(env.store.is_unit(u));
        let n = env.builtin("Nothing").unwrap();
        assert!(env.store.is_nothing(n));
        assert!(env.builtin("NotAType").is_none());
        assert_eq!(env.builtin("Byte").unwrap(), env.builtin("UInt8").unwrap());
        assert_eq!(
            env.builtin("Double").unwrap(),
            env.builtin("Float64").unwrap()
        );
        let _ = it;
    }
}
