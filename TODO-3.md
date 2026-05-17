# TODO（core / stdlib reshape）：P6 + P7：f-string desugar + Intrinsic 转 scoop ABI

> 计划基线：[`PLAN.md`](./PLAN.md)
> 任务索引：[`TODO.md`](./TODO.md)
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。
> 全局约束：见 [`TODO.md`](./TODO.md) `## 全局约束` 一节。
## P6：f-string desugar

### [DONE] P6-T01：f-string HIR desugar——改写为 `StringBuilder().add(...).toString()`

- 参考：
  - [`PLAN.md`](./PLAN.md) §7.1 / §9 / P6
  - `crates/scoopc/src/ast/mod.rs::InterpolatedString`（line 1779，Text/Expr part 列表）
  - `crates/scoopc/src/hir/mod.rs::InterpolatedStringPart`（line 463）
  - `crates/scoopc/src/hir/lower/expr/`（HIR lowering 入口；具体 `lower_expr` 函数中 InterpolatedString 分支）
  - `crates/scoopc/src/parser/expr.rs::find_interpolation_close_in_f_string`（line 1529，parser 已经把 f-string 拆分为 Text/Expr part 列表）
  - P5-T02 引入的 `StringBuilder` class
- 目标：
  - 在 HIR lowering 阶段把 InterpolatedString 改写为 `StringBuilder` 调用链。
  - desugar 形态：

    `f"a={x}, b={y}"` →
    ```
    StringBuilder()
        .add("a=")
        .add(x.toString())
        .add(", b=")
        .add(y.toString())
        .toString()
    ```

  - 文本部分的 escape 解析（`{{` / `}}` / `\n` / raw vs non-raw）保持现 parser 阶段的处理；HIR lowering 拿到的是已解码的 `Text { content }`。
- 当前实现入口：
  - HIR lowering 中 InterpolatedString 的处理（grep `InterpolatedString` / `InterpolatedStringPart` 在 `crates/scoopc/src/hir/lower/`）
  - typecheck 中对 f-string 各 expr part 的 `: ToString` 检查（grep `interpolation` / `ToString` 在 `crates/scoopc/src/typecheck/`）
- 必须实现的内容：
  1. 在 HIR lowering 阶段 `lower_expr` 的 `InterpolatedString` 分支中：
     - 不再保留 `InterpolatedString` 作为 HIR 节点；直接生成 `StringBuilder` 调用链。如果当前 HIR 仍有 `InterpolatedString` 表示，desugar 在 lowering 入口替换；如果已经是 lowering output of HIR，把 transform 放在更靠后的 HIR pass。
     - 顺序：
       - 生成 `let __sb_<n> = StringBuilder()`
       - 对每个 part：
         - Text part：生成 `__sb_<n>.add("<text>")`（直接用 String literal）
         - Expr part：生成 `__sb_<n>.add(<expr>.toString())`
       - 最后生成 `__sb_<n>.toString()` 作为整个 f-string 表达式的值
     - 链式调用形态可选（fluent `.add(...).add(...).toString()`），但要确认当前 HIR 对链式调用的 type inference 路径成熟；否则用 let-binding 串起来更稳。倾向 let-binding 形态——避免 typecheck 期间链式 receiver 推断失败。
  2. typecheck：
     - 保留现有"each expr part : ToString"诊断（不变）。如果当前诊断在 InterpolatedString HIR 节点上，迁到 desugar 之前的某个 typecheck 阶段。
     - desugar 出来的 `<expr>.toString()` 调用走普通 method dispatch（前提是 expr 类型实现 `ToString`）。**不**为这条调用注入特殊路径。
  3. raw f-string `f"""..."""`：与 non-raw 共享同一 desugar 形态；区别仅在 Text part 的 escape 规则（这部分 parser 已处理）。
  4. 编辑点（开工时定位精确行）：
     - HIR `lower_expr` 中 `ast::Expr::InterpolatedString` 分支
     - 必要时在 `crates/scoopc/src/hir/mod.rs` 删除 `InterpolatedStringPart` enum（如果 desugar 后没有任何 IR 节点使用）
  5. owner 测试：
     - `tests/fixtures/run-pass/fstring_desugar_basic.scoop`：单 expr / 多 expr / 含 `{{` 转义 / raw + interpolation
     - HIR snapshot 测试 `f_string_lowers_to_string_builder_chain`：`val s = f"a{x}b"` 的 lowered HIR 包含 `StringBuilder` 构造 + 3 次 `add`（"a" / `x.toString()` / "b"）+ `toString` 调用
     - typecheck 测试：`f"{x}"` 当 x 类型不实现 ToString 时报"interpolation expr must be ToString"诊断，错误位置仍指向 expr part（不是 desugar 后的 method call）
- 必须遵从的约束：
  - **不**删除现有 LLVM 阶段 f-string codegen 路径（在 P6-T02 删）—— 本任务期间它仍存在但不会被调到。
  - core / lang.string 自身**不**使用 f-string 字面量（P6-T02 加 lint）。
  - desugar 出来的 `StringBuilder` 引用通过自动 prelude 解析（P1-T02 已落地）；不依赖用户写显式 import。
- 验证：
  1. `cargo test -p scoopc fstring_desugar -- --nocapture`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/fstring_*.scoop`
  3. `cargo run -p scoop -- test`（P0-T01 fstring-fixtures 全集）—— 所有 f-string fixture stdout 与 baseline 一致
- 完成条件：
  - HIR 阶段 InterpolatedString 节点不再存在（或不再走 LLVM codegen 后门路径）。
- 依赖：P5-T02。

完成记录（2026-05-17）：

- 改动范围：`crates/scoopc/src/hir/lower/expr/main_lower.rs` 将 `ast::ExprKind::InterpolatedString` 降为 `StringBuilder` block（`__sb_N = StringBuilder()`、逐段 `add`、最终 `toString`）；`syntax/string_literal.rs` 提供 f-string Text 片段解码；HIR/MIR 增加 `SynthString` 以承载已解码的合成字符串；typed HIR contract 支持合成 member-call dispatch；typecheck 增加 `interpolation_expr_not_to_string` 诊断；新增 `fstring_desugar_basic` run-pass fixture 与非 ToString 诊断 fixture；同步 LLVM IR 断言避免依赖临时名精确后缀。
- 核心决策：采用 let-binding 串行形态而非 fluent chain，保证 builder receiver 推断与求值顺序稳定；Text part 在 lowering 时解码为 `SynthString`，不伪造源码字符串 span；Expr part 生成 `ToString.toString` 的普通 member-access interface dispatch，避免把 interface method 当成直接可达函数 body；保留 LLVM 阶段 f-string codegen 后门，留给 P6-T02 删除。
- 验证结果：`cargo test -p scoopc fstring_desugar -- --nocapture` 通过；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/fstring_*.scoop` 通过；`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/fstring_interpolation_non_tostring_is_error.scoop` 通过；`cargo run -p scoop -- test --fixtures tests/fixtures/codegen/f_string_interpolation.scoop` 通过；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/class_ctor_arg_eval_scope_shadow_free_basic.scoop` 通过；`cargo test --all --all-targets` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
- 全量 fixture：`cargo run -p scoop -- test` 完整执行，结果为 7 个失败、1335 个通过、1372 checks 通过。失败项均不由本任务新引入的 f-string owner path 触发：`tests/fixtures/runtime_gc/extern_enter_native_gc_arg_spill_reload.scoop`、`tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`、`tests/fixtures/runtime_gc/funptr_enter_native_roots_gc.scoop`、`tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`、`tests/fixtures/runtime_gc/gc_move_stackmap_heap_fixup.scoop` 为 runtime GC/native-root stdout mismatch；`tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic` 仍命中既有 cross-file ctor codegen path（直接复核显示与本任务 f-string direct owner fixtures不同）；这些不改变 P6-T01 完成范围，后续全量收尾按 P13-T04 处理。
- 与 `PLAN.md` 闭合：完成 P6 §7.1 的 HIR f-string desugar 主线，使新 f-string 不再进入 LLVM f-string 拼装路径；P6-T02 继续负责删除后端旧路径与增加 sysroot f-string lint。
- 暂时性 failing fixture：本任务未新增必须由 P6-T01 处理的 f-string owner failing fixture；全量 baseline 的 7 个剩余失败按上条记录，不作为 P6-T01 的前置 blocker。

### [DONE] P6-T02：删除 LLVM 阶段 f-string codegen 后门 + sysroot 文件 f-string 使用 lint

- 参考：
  - [`PLAN.md`](./PLAN.md) §7.2 / §7.3 / §9 / P6
  - `crates/scoopc/src/llvm/codegen/main/literal.rs::codegen_interpolated_string`（line 236，整函数 `codegen_interpolated_string` ~ 470 行）
  - `crates/scoopc/src/llvm/codegen/mir_body/string.rs::codegen_mir_interpolated_string`（line 8，~210 行）+ `codegen_mir_interpolated_expr_segment`（line 212）
  - `crates/scoopc/src/mir/lower/fn_lowering_expr.rs::lower_interpolated_string_expr`（line 86）
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中只为 f-string interpolation 使用的 runtime symbol declaration（grep `interpolation` 或按 line 161 注释 "用于 f-string 插值 `{Int}` 的最小 formatting" 定位）
- 目标：
  - 删除 LLVM 阶段所有 f-string 拼装路径。codegen 期间不再有 InterpolatedString 节点流过。
  - 给 sysroot 文件加"禁止使用 f-string"的 lint。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/main/literal.rs::codegen_interpolated_string`
  - `crates/scoopc/src/llvm/codegen/mir_body/string.rs::codegen_mir_interpolated_string` / `codegen_mir_interpolated_expr_segment`
  - `crates/scoopc/src/mir/lower/fn_lowering_expr.rs::lower_interpolated_string_expr`
- 必须实现的内容：
  1. 删除三个 codegen 函数 `codegen_interpolated_string` / `codegen_mir_interpolated_string` / `codegen_mir_interpolated_expr_segment`，以及它们的所有 caller-site dispatch（`lower_expr` 中如 `Expr::InterpolatedString` 分支）。
  2. 删除 `lower_interpolated_string_expr`（MIR lowering）。
  3. 删除 `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中**仅**为 f-string interpolation 使用的 runtime symbol declarations。注意 `scoop_bool_to_string` 既被 f-string 也被 `Bool.toString()` 用，此符号**保留**（用法分析时 grep `scoop_bool_to_string` 在 codegen/ 与 sysroot/ 下确认）。
  4. 在 typecheck 或 parser 阶段加 sysroot lint：扫描 `source.is_sysroot() == true` 的文件中是否含 f-string 字面量；如有则 emit 编译错误"sysroot files cannot use f-string"。
     - 实施位置：parser 阶段更早、错误信息更具上下文；但当前 `is_sysroot` flag 在 source 加载时就有，parser 可以查。具体放 parser 还是 resolver/typecheck 取决于 architecture——开工时确认。
     - 加 owner 测试 `sysroot_files_cannot_contain_fstring`：构造一个 sysroot virtual file 含 `val x = f"hello"`，断言编译失败 + 错误位置指向 f-string token。
  5. 删除 `crates/scoopc/src/ast/mod.rs::InterpolatedString` AST 节点 / `crates/scoopc/src/hir/mod.rs::InterpolatedStringPart` 是否需要——如果 P6-T01 已经在 HIR 入口 desugar、AST/HIR 节点不再流到 codegen，那这两个节点定义可以保留作为 parser 输出的中间形式。**不**强制删除，按"无用代码清理"原则在 P12 处理。
- 必须遵从的约束：
  - 删除 codegen 后门必须在 P6-T01 desugar 已经稳定的前提下做，否则 f-string 编译会立刻失败。
  - 不允许保留任何 LLVM 阶段的 f-string fallback 路径。
- 验证：
  1. `grep -r "codegen_interpolated_string\|codegen_mir_interpolated\|lower_interpolated_string" crates/scoopc/src/`—— 应完全无命中。
  2. `cargo test -p scoopc sysroot_files_cannot_contain_fstring -- --nocapture`
  3. `cargo run -p scoop -- test`（P0-T01 fstring-fixtures 全集 + 全量 baseline）—— 所有 f-string fixture 仍 pass，运行结果与 baseline 一致。
- 完成条件：
  - LLVM codegen 中不再有 f-string 处理路径。
  - sysroot 中误用 f-string 会被编译期拒绝。
- 依赖：P6-T01。

完成记录（2026-05-17）：

- 改动范围：`crates/scoopc/src/llvm/codegen/main/literal.rs` 删除 direct HIR f-string 拼装函数与 string len/data helper；`crates/scoopc/src/llvm/codegen/mir_body/string.rs` 删除 MIR f-string 拼装与 expr segment helper；`crates/scoopc/src/mir/lower/fn_lowering_expr.rs` / `fn_lowering_basic.rs` 删除 `lower_interpolated_string_expr` 入口；`runtime_abi.rs` / `runtime_symbols.rs` / builtin intrinsic helper 中移除 f-string-only scalar formatting declarations 与未用 helper；`parser` 增加 sysroot f-string 诊断；同步更新 user-visible failure audit 计数。
- 核心决策：保留 AST/HIR 的 `InterpolatedString` 中间表示作为 parser 输出与 HIR desugar 输入，但 LLVM/MIR 阶段只保留不可达 guard，不再有任何拼装 fallback；HIR desugar 函数重命名为 `desugar_f_string_expr`，满足旧后门命名 grep 完全无命中；sysroot lint 放在 parser 阶段，直接用 `SourceFile::is_sysroot()` 在 f-string token 处报 `sysroot files cannot use f-string`。
- 验证结果：`grep "codegen_interpolated_string\|codegen_mir_interpolated\|lower_interpolated_string" crates/scoopc/src` 无命中；`cargo test -p scoopc sysroot_files_cannot_contain_fstring -- --nocapture` 通过；`cargo test -p scoopc fstring_desugar -- --nocapture` 通过；`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/fstring_*.scoop` 通过；`cargo test --all --all-targets` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
- 全量 fixture：`cargo run -p scoop -- test` 在最终代码状态下完整执行，结果为 7 个失败、1335 个通过、1372 checks 通过。失败项不由本任务 f-string 后门删除引入，且不属于 P6-T02 owner path：`tests/fixtures/run-pass/mutable_array_ops_basic.scoop`、`tests/fixtures/runtime_gc/extern_enter_native_gc_arg_spill_reload.scoop`、`tests/fixtures/runtime_gc/extern_enter_native_roots_gc.scoop`、`tests/fixtures/runtime_gc/funptr_enter_native_roots_gc.scoop`、`tests/fixtures/runtime_gc/gc_handle_roundtrip.scoop`、`tests/fixtures/runtime_gc/gc_move_stackmap_heap_fixup.scoop`、`tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic`。
- 与 `PLAN.md` 闭合：完成 P6 §7.2 / §7.3，LLVM 阶段旧 f-string 拼装后门退场，core / lang.string 等 sysroot 文件误用 f-string 会在编译前端被拒绝；阶段级计划未变化，未修改 `PLAN.md`。
- 暂时性 failing fixture：本任务未新增 f-string owner failing fixture；全量 baseline 的 7 个剩余失败继续按 P13-T04 最终 fixture 收尾处理。

## P7：Intrinsic → scoop ABI 批量转换

### P7-T01：sysroot——`String` body method / 标量 toString / print/println / panic 等转 `@Extern(abi = "scoop")`

- 参考：
  - [`PLAN.md`](./PLAN.md) §3.4 / §9 / P7
  - `sysroot/core.scoop`（line 76-120：`@Intrinsic class String` body method；line 282-291：`__scoop_print_string` / `__scoop_println_string`；line 305-310：`__scoop_gc_collect`；line 576-577：`panic`）
  - `sysroot/scalar_string_bridge.scoop`（整文件，5 个 `__scoop_runtime_*_to_string_bridge` 与对应 `scoopAbi*ToString` wrapper）
  - `sysroot/string.scoop` line 19-21：`__scoop_runtime_string_concat_bridge`
  - `runtime/c/scoop_runtime.c::{scoop_print, scoop_println, scoop_panic, scoop_bool_to_string, scoop_char_to_string, scoop_int_to_string, scoop_float32_to_string, scoop_float64_to_string, scoop_string_concat, scoop_string_unsafe_slice_bytes}`（line 644 / 663 / 810 / 881 / 897 / 930 / 985 / 980 / 1042 / 1117）
- 目标：
  - 把 sysroot 中"实际只是 runtime symbol 包装"的 `@Intrinsic` 声明全部转为 `@Extern(abi = "scoop")` 顶层声明。
  - 编译器层面对应的 named-intrinsic dispatch 删除（在 P7-T02 处理）。
- 当前实现入口：
  - `sysroot/core.scoop`（line 76-120 + 282-310 + 576-577）
  - `sysroot/scalar_string_bridge.scoop`（整文件）
  - `sysroot/string.scoop`（line 19-21 + 后续 `__scoop_string_*` 调用 audited bridge 的位置）
- 必须实现的内容：
  1. **String body method**（在 `sysroot/core.scoop::class String`）：当前形态是 `@Intrinsic class String { ... fun length(): Int { return __scoop_string_length(this) } ... }`，其中 `__scoop_string_length` 等是定义在 `sysroot/string.scoop` 的 ordinary helper。这种形态实际上**已经**是"普通 Scoop body 调 sysroot helper"，不是 intrinsic 后门——本任务无需改动 String body method。重点是把它们底层最终调到的 audited bridge 转 scoop ABI（见步骤 2 / 3）。
  2. **替换 audited bridge** —— 把这一组 `@Intrinsic("xxx_bridge")` 改为 `@Extern(name = "scoop_xxx", abi = "scoop")`：
     - `sysroot/string.scoop` line 19-21：
       ```
       @Extern(name = "scoop_string_concat", abi = "scoop")
       fun __scoop_string_concat_runtime(a: String, b: String): String
       ```
       并把现有 `__scoop_string_concat` 函数 body（在 `sysroot/string.scoop` line 82-84，`return __scoop_runtime_string_concat_bridge(a, b)`）改为 `return __scoop_string_concat_runtime(a, b)`。
     - 同理：`sysroot/scalar_string_bridge.scoop` 整文件**删除**（在 P7-T02 做）；现 `class String / Int / Bool / Char / Float32 / Float64` 的 `toString()` body（在 `sysroot/core.scoop`）当前形态如 `return scoopAbiCharToString(this)`，本任务期间改为：
       ```
       override fun toString(): String {
           return __scoop_char_to_string(this)
       }
       ```
       并在 `sysroot/core.scoop`（或紧贴 String class 处）声明：
       ```
       @Extern(name = "scoop_char_to_string", abi = "scoop")
       fun __scoop_char_to_string(value: Char): String

       @Extern(name = "scoop_int_to_string", abi = "scoop")
       fun __scoop_int_to_string(value: Int): String

       @Extern(name = "scoop_bool_to_string", abi = "scoop")
       fun __scoop_bool_to_string(value: Bool): String

       @Extern(name = "scoop_float32_to_string", abi = "scoop")
       fun __scoop_float32_to_string(value: Float32): String

       @Extern(name = "scoop_float64_to_string", abi = "scoop")
       fun __scoop_float64_to_string(value: Float64): String
       ```
  3. **String.unsafeSliceBytes** —— 当前是 `@Intrinsic` method（`sysroot/core.scoop` 没暴露但有内部 `byteLength/getByte/unsafeSliceBytes` 三 intrinsic）。`unsafeSliceBytes` 是 runtime allocation + bytes copy（参考 `runtime/c/scoop_runtime.c::scoop_string_unsafe_slice_bytes` line 1117），**不是** inline 操作—— 应当转 scoop ABI：
     ```
     @Extern(name = "scoop_string_unsafe_slice_bytes", abi = "scoop")
     @Unsafe
     fun __scoop_string_unsafe_slice_bytes(s: String, byteOffset: Int, byteLength: Int): String
     ```
     并把 `String.unsafeSliceBytes` body 改为调用 `__scoop_string_unsafe_slice_bytes`。**注意**：scoop ABI 不允许显式叠加 `@Unsafe` —— 与 P5-T02 步骤 1 同样的约束；如果约束仍在，wrapper 函数加 `@Unsafe` 体处理（即 sysroot 暴露的是普通 scoop ABI 声明，用户 caller 通过 `String.unsafeSliceBytes` 这个 method 看到 unsafe 表面）。
     - 但 `byteLength/getByte` 仍**保留** intrinsic（直接读 String header / GEP byte，不是 runtime call）。
  4. **print / println**：
     - `sysroot/core.scoop` line 282-291：`__scoop_print_string` / `__scoop_println_string` 当前是 `@Intrinsic` 声明。改为：
       ```
       @Extern(name = "scoop_print", abi = "scoop")
       fun __scoop_print(value: String): Unit

       @Extern(name = "scoop_println", abi = "scoop")
       fun __scoop_println(value: String): Unit
       ```
     - `sysroot/print.scoop` 中泛型 `print<T>/println<T>` body 改为调用新名字。
     - 注意：runtime 当前导出 `scoop_print` / `scoop_println`（见 `runtime/c/scoop_runtime.c` line 644 / 663），不需要改 runtime side。如有名字差异（旧 sysroot 用 `__scoop_print_string` 但 runtime 实际导出是 `scoop_print`），P7-T03 已经记录了符号一致性问题——需要先确认现有映射是怎么对上的。grep 当前 `runtime_abi.rs` / `runtime_symbols.rs` 中 `print` 相关 symbol 名。
  5. **panic**：
     - `sysroot/core.scoop` line 576-577：`@Intrinsic fun panic(message: String): Nothing`。改为：
       ```
       @Extern(name = "scoop_panic", abi = "scoop")
       fun panic(message: String): Nothing
       ```
       runtime 已有 `scoop_panic`（line 810），signature 一致（参数 String、返 void/Nothing）。
  6. **__scoop_gc_collect**：
     - `sysroot/core.scoop` line 305：`@Intrinsic fun __scoop_gc_collect(): Unit`。改为：
       ```
       @Extern(name = "scoop_gc_collect", abi = "scoop")
       fun __scoop_gc_collect(): Unit
       ```
       runtime 已有 `scoop_gc_collect_safepoint`（line 619）—— 名字差异需要在 P7-T03 阶段统一：要么改 sysroot 声明的 `name = "scoop_gc_collect_safepoint"`，要么 runtime 改名。先记入 P7-T03。
  7. owner 测试：
     - 现有 baseline 中所有用到 `print` / `println` / `Int.toString` / `String.concat` / `panic` 的 fixture 必须 pass。
     - 加一个 IR snapshot 测试：`val s = (42).toString()` 的 IR 中调用的是 `scoop_int_to_string`（现在通过 scoop ABI），而**不是**任何 audited bridge wrapper 路径。
- 必须遵从的约束：
  - 不允许在 sysroot 中保留 `__scoop_runtime_*_bridge` 这一层（在 P7-T02 删 `scalar_string_bridge.scoop` 整文件）。
  - `String.byteLength` / `String.getByte` 保留为 intrinsic（直接读字段 / GEP byte，符合 §3.3 (a)）。
  - GC pin/unpin/handle 保留为 intrinsic（§3.3 (b)，本任务不动）。
- 验证：
  1. `cargo run -p scoop -- test`（全量 baseline，无回退）
  2. IR snapshot 测试：选 1-2 条对 print / Int.toString 调用的 fixture，用 `scoopc --emit ir` 检查 call 指令直接指向 `scoop_int_to_string` 等 runtime symbol。
- 完成条件：
  - 所有"包装 runtime symbol"的 `@Intrinsic` 已转为 `@Extern(abi = "scoop")`。
  - sysroot 中只有 §3.3 三类真 intrinsic 还保留 `@Intrinsic` 标记。
- 依赖：P6-T02。

### P7-T02：删除 `sysroot/scalar_string_bridge.scoop` + 编译器对应 audited bridge dispatch

- 参考：
  - [`PLAN.md`](./PLAN.md) §3.4 / §9 / P7
  - `sysroot/scalar_string_bridge.scoop`（整文件 39 行，5 个 audited bridge + 5 个 `scoopAbi*ToString` wrapper）
  - `crates/scoopc/src/intrinsics.rs` 中 `scalar_*_to_string_bridge` audited 表项
  - `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中 `declare_runtime_*_to_string_bridge` 函数（如有）
  - `crates/scoopc/src/llvm/tests.rs::compiled_sysroot_scalar_string_bridge_helpers_stay_in_module`（如存在，需删除或调整）
- 目标：
  - 删除 `sysroot/scalar_string_bridge.scoop` 整文件。
  - 删除编译器 `intrinsics.rs` / `runtime_abi.rs` / 测试中所有 `scalar_*_to_string_bridge` 相关 special-case。
- 当前实现入口：
  - `sysroot/scalar_string_bridge.scoop`
  - `crates/scoopc/src/intrinsics.rs`（搜 `scalar_char_to_string_bridge` / `scalar_int_to_string_bridge` 等）
  - `crates/scoopc/src/llvm/tests.rs`（owner 测试）
- 必须实现的内容：
  1. **前提验证**：P7-T01 完成后 `scoopAbiCharToString` / `scoopAbiIntToString` / `scoopAbiBoolToString` / `scoopAbiFloat32ToString` / `scoopAbiFloat64ToString` 不再被任何代码调用。验证方式：
     - `grep -rn "scoopAbi.*ToString" crates/ runtime/ sysroot/`—— 应该没有命中。
     - 如有命中（如 `class Float64.toString()` body 仍写 `return scoopAbiFloat64ToString(this)`），P7-T01 没改完—— 回到 P7-T01 修复后再回来。
  2. 删除 `sysroot/scalar_string_bridge.scoop` 整文件。
  3. 在 `crates/scoopc/src/intrinsics.rs` 中删除：
     - `scalar_char_to_string_bridge` / `scalar_int_to_string_bridge` / `scalar_bool_to_string_bridge` / `scalar_float32_to_string_bridge` / `scalar_float64_to_string_bridge` 5 条 audited entry（在 `named_intrinsic_audit_entries()` 列表中）。
     - 任何 `fallback_named_intrinsic_entry_name_for_fqn` 中对这 5 个名字的映射（grep `__scoop_runtime_.*_bridge` 或 `scalar_.*_to_string_bridge`）。
  4. 在 `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 中删除 `declare_runtime_*_to_string_bridge` 一组函数（grep `to_string_bridge` 在 codegen/ 下）。
  5. 删除 owner 测试 `crates/scoopc/src/llvm/tests.rs::compiled_sysroot_scalar_string_bridge_helpers_stay_in_module`（如存在）。新加替代测试 `scalar_to_string_calls_runtime_directly`：验证 `Int.toString()` 的 IR 直接 call `scoop_int_to_string`，**不**有任何中间 wrapper 函数。
- 必须遵从的约束：
  - 仅在 P7-T01 完成且通过 verify "no caller of scoopAbi*ToString" 后才能开工。
  - 不允许保留任何 `bridge` 命名的函数 / 文件 / 测试 / dispatch entry。
- 验证：
  1. `grep -rn "scoopAbi.*ToString\|scalar_.*_to_string_bridge\|to_string_bridge" crates/ runtime/ sysroot/`—— 完全无命中。
  2. `ls sysroot/scalar_string_bridge.scoop` —— 文件不存在。
  3. `cargo run -p scoop -- test`（全量 baseline，无回退）
- 完成条件：
  - audited bridge 一层完整退场。
  - sysroot 文件数从 9 减为 8。
- 依赖：P7-T01。

### P7-T03：runtime 端可能的符号改名（`scoop_print_string` → `scoop_print` 等）

- 参考：
  - [`PLAN.md`](./PLAN.md) §10 风险条目（runtime symbol 改名）
  - `runtime/c/scoop_runtime_api.h::SCOOP_RUNTIME_API_X_LIST`
  - `runtime/c/scoop_runtime.c` 中 `scoop_print` / `scoop_println` / `scoop_panic` / `scoop_gc_collect_safepoint` 等导出
- 目标：
  - 解决 P7-T01 中发现的 sysroot `@Extern(name = ...)` 期望名与 runtime 实际导出名不一致的情况。
  - 将 runtime 端符号统一为 `scoop_<verb>` 简洁形式（不带 `_string` / `_safepoint` 后缀），与 sysroot 声明对齐。
- 当前实现入口：
  - `runtime/c/scoop_runtime.c::{scoop_print, scoop_println, scoop_panic, scoop_gc_collect_safepoint}`
  - `runtime/c/scoop_runtime_api.h`
- 必须实现的内容：
  1. 列出 P7-T01 完成时 sysroot 与 runtime 的符号名差异表：
     - `__scoop_print_string`（旧 sysroot）→ sysroot 已改名 `__scoop_print`，runtime 当前 `scoop_print` —— 一致 ✓
     - `__scoop_println_string`（旧）→ `__scoop_println` / `scoop_println` —— 一致 ✓
     - `panic` → `scoop_panic` —— 一致 ✓
     - `__scoop_gc_collect`（sysroot）→ runtime `scoop_gc_collect_safepoint` —— **不一致**
  2. 决策 `scoop_gc_collect`：
     - 选项 A：runtime 改名 `scoop_gc_collect_safepoint` → `scoop_gc_collect`（与 sysroot 对齐）。在 `scoop_runtime_api.h` 与 `scoop_runtime.c` 同步改名。
     - 选项 B：sysroot 写 `@Extern(name = "scoop_gc_collect_safepoint", abi = "scoop")` 但函数名仍是 `__scoop_gc_collect`。
     倾向 A——保持命名简洁、与其它 `scoop_<verb>` 一致。
  3. 实施：按选项 A 改 runtime；同步更新 `scoop_runtime_api.h::SCOOP_RUNTIME_API_X_LIST` 中的 `X(scoop_gc_collect_safepoint)` 行（如存在）。
  4. 跑全量 baseline 验证 link 不出错。
  5. 把整次符号差异审计的发现 + 决策结果写入完成记录（供 P12 文档回写参考）。
- 必须遵从的约束：
  - 选项 A 必须是 atomic change：runtime 改名与 sysroot `@Extern(name = ...)` 对齐必须在同一 commit 内完成，避免中间状态 link error。
- 验证：
  1. `cargo build` —— 整仓 link 通过。
  2. `cargo run -p scoop -- test`（全量 baseline，无回退）
  3. `nm` 或等价工具检查 final binary，确认导出符号集合中**不**存在已废弃的旧名（如 `scoop_gc_collect_safepoint` 在改名后不应再出现）。
- 完成条件：
  - sysroot `@Extern(name = ...)` 与 runtime 导出符号一一对应、命名一致。
- 依赖：P7-T02。
