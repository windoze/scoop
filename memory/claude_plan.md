# Current Task: T0124 — 泛型验证与修复：monomorphization 扩展至泛型 struct

## Status: DONE

## Completed Work
- [x] mangled FQN + 泛型 struct/enum 布局收集 (`mangle_nominal_fqn`, `collect_generic_struct_instantiation_layouts`, `collect_generic_enum_instantiation_layouts`)
- [x] codegen 所有布局查找改用 mangled FQN (`cg_ty_of`, `llvm_struct_type`, `codegen_struct_lit`, `lookup_struct_field`, `struct_clayout`, GC-free checks)
- [x] typecheck 修复：`push_type_params` for generic struct field types + 跳过泛型类型的跨文件构造函数路径
- [x] generic struct literal 类型推断：`infer_generic_struct_lit_expr_type()` + expected type 上下文传递
- [x] HIR lowering 修复：`ExpectedExpr.struct_lit_ty` 传递 val 声明类型给 struct literal lowering
- [x] Fixture: `generic_struct_basic.scoop` — `Pair<Int, Int>` + `Pair<Int, String>` 多实例化
- [x] 全部测试通过 (`cargo test --all` + `cargo run -p scoop -- test` 810 fixtures)
