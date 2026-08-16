//! 全局初始化规划：读取 `Item::Initializer`，构建 GlobalInitPlan + ClassInitPlan。

use scoop2_base::Interner;
use scoop2_mir::mir::Item;
use scoop2_mir::mir::materialize::MaterializedMir;

use crate::*;

/// 主入口：构建顶层 val/var 初始化计划 + 类初始化计划。
pub fn plan_global_init(
    program: &mut LirProgram,
    mir: &MaterializedMir,
    decls: &scoop2_mir::mir::decls::MirDecls,
    interner: &Interner,
) {
    // 7a: 顶层 val/var 初始化。
    //
    // 初始化条目按 MIR `module.items` 的声明顺序保留。Scoop 的类型检查器强制要求
    // 顶层 `val`/`var` 必须在引用前声明（MIR lowering 沿源声明顺序产出
    // `Item::Initializer`），因此 MIR item 顺序即为正确的初始化依赖序：
    // 若 `a` 的初始化器引用 `b`，则 `b` 在源中先于 `a` 声明，因而 `b` 的
    // Initializer item 也排在 `a` 之前。此处无需再做拓扑排序——声明顺序天然满足
    // 「被引用者先初始化」的不变式。
    let mut entries: Vec<GlobalInitEntry> = Vec::new();
    for item in &mir.module.items {
        if let Item::Initializer(ir) = item {
            // M3-2：MIR 定稿符号（纯读）。
            let init_callable = if ir.symbol.is_empty() {
                ir.fqn.replace('.', "_")
            } else {
                ir.symbol.clone()
            };
            entries.push(GlobalInitEntry {
                fqn: ir.fqn.clone(),
                ty: ir.ty,
                is_var: ir.is_var,
                init_callable,
            });
        }
    }
    program.global_init = GlobalInitPlan { entries };

    // 7b: 类初始化计划。
    // 遍历 BackendContracts.class_inits + HIR 类型声明构建 ClassInitPlan。
    for class_init in &mir.backend_contracts.class_inits {
        let class_fqn_text = &class_init.class_fqn;
        // 查 HIR 获取 class 的字段列表（超类字段在前，自身字段按 member_order
        // 声明序——与 LIR compute_field_offset / codegen class ctor 布局一致）。
        let fqn_sym = interner.get(class_fqn_text);
        let mut field_inits: Vec<FieldInit> = Vec::new();
        if let Some(sym) = fqn_sym {
            for (member_name_sym, member_ty) in decls.ordered_class_fields(sym) {
                let field_name = interner.resolve(member_name_sym).to_string();
                // InitKind 区分：理想情况下，主构造器参数对应的字段应为
                // PropertyParam，声明处带初始化器的属性应为 PropertyInitializer，
                // 其余为 DefaultValue。但当前 MIR BackendContracts 未暴露
                // 构造器参数名 / 属性初始化器来源，无法在此准确区分，故统一用
                // DefaultValue（零初始化）。待 ctor signatures 导出后可精确归类。
                field_inits.push(FieldInit {
                    field_name,
                    ty: member_ty,
                    init_kind: InitKind::DefaultValue,
                });
            }
        }
        // 查找超类：从 HIR supertypes 表取第一个 class 超类型（与
        // `ordered_class_fields` 的超类链同源；替代旧的 vtable 启发式——
        // 超类无虚方法可继承时 vtable 推断会丢失 super_init）。
        let super_init = fqn_sym.and_then(|sym| {
            decls
                .supertypes_of(&sym)
                .iter()
                .find(|s| decls.is_class(s))
                .map(|s| interner.resolve(*s).to_string())
        });
        program.class_inits.push(ClassInitPlan {
            class_fqn: class_fqn_text.clone(),
            field_inits,
            super_init,
            // init_blocks：主构造器逻辑在 LIR 中尚未展开为独立 callable。
            // 待构造器 lowering 暴露 per-block callable 后填充。
            init_blocks: Vec::new(),
        });
    }
}
