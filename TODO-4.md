# TODO（core / stdlib reshape）：P8：算术 / 逻辑 operator method 化

> 计划基线：[`PLAN.md`](./PLAN.md)
> 任务索引：[`TODO.md`](./TODO.md)
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。
> 全局约束：见 [`TODO.md`](./TODO.md) `## 全局约束` 一节。
## P8：算术 / 逻辑 operator method 化

### [DONE] P8-T01：标量 operator behavioral baseline 短文

- 参考：
  - [`PLAN.md`](./PLAN.md) §3.3 (a) / §9 / P8
  - `crates/scoopc/src/llvm/codegen/mir_body/op.rs`（line 8 `codegen_mir_unary`、line 58 `codegen_mir_binary`，文件 408 行）
  - `crates/scoopc/src/hir/lower/expr/members.rs::lower_binary_expr_type`（line 1402-1480）
  - C / Rust / Kotlin / Java 在对应运算上的边界值约定（仅作对照参考；最终基线以当前实现为准）
- 目标：
  - 在 `docs/reshape-baseline/operator-behavioral-baseline.md` 写一份"当前 operator codegen 的逐 op 边界值行为"白皮书。后续 P8-T02 ~ P8-T05 的 method intrinsic lowering 必须**逐位一致**。
  - 这一文档是 P8 实现期间的唯一仲裁依据；任何"顺手修一下行为"的提议必须独立 PR。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/mir_body/op.rs` 全文件
  - 现有算术 fixture（grep `tests/fixtures/run-pass/` 中含 `+ - * / % << >> & | ^ ~ ! < <= > >= == != && ||` 的 fixture）
- 必须实现的内容：
  1. 把 `mir_body/op.rs` 中每条 LLVM 指令选择规则汇总成表，并对每行写"参数极值的输出"。覆盖：
     - **整型 plus/minus/times**：`add/sub/mul`，二补码 wrap-around 行为（无 overflow trap）。`Int.MIN_VALUE + (-1)` / `Int.MIN_VALUE * 2` 等极端情形的实际输出。
     - **整型 div/rem**：
       - signed：`sdiv` / `srem`。LLVM `sdiv X / 0` 是 UB；当前是否在 codegen 前插 zero check？检查 `mir_body/op.rs` line 88-110 与上层 typecheck/mir 阶段。如果当前**没有** zero check，记下"divide by zero 是 UB"；如果有，记下 zero check 的形态（trap / panic / Raise）。
       - `Int.MIN_VALUE / -1`：LLVM `sdiv` 在此情形也是 UB。同上记下当前行为。
       - unsigned：`udiv` / `urem`。
     - **整型 shl/shr**：
       - `shl`：amount >= bit_width 是 LLVM UB。当前是否做 mask？记下。
       - signed `shr`：`ashr`（保符号）。
       - unsigned `shr`：`lshr`（zero fill）。
       - amount 为负的处理（如果允许 signed shift amount）。
     - **整型 bitwise**：`and/or/xor`，对所有位组合行为明确，无歧义。
     - **整型 unary minus**：`sub 0, x`。`Int.MIN_VALUE.unaryMinus()` 在 `signed wrap` 下仍是 `Int.MIN_VALUE`（值不变）。记下当前是否如此。
     - **整型 compare**：6 个 predicate（lt/le/gt/ge/eq/ne）按 signed/unsigned 选 `slt/ult` 等。
     - **浮点 plus/minus/times/div/rem**：`fadd/fsub/fmul/fdiv/frem`。
       - NaN 传播：任一 operand 是 NaN，结果是 NaN。
       - `+inf + -inf = NaN`。
       - `0.0 / 0.0 = NaN`。
       - `1.0 / 0.0 = +inf`。
       - `+0.0 == -0.0` 是 true（`fcmp oeq`）但 `bit_pattern(+0.0) != bit_pattern(-0.0)`。
     - **浮点 unary minus**：`fneg`。`-NaN` 仍是 NaN（sign bit 翻转，payload 不变）。
     - **浮点 compare**：6 个 predicate，按 NaN-aware predicate 选（`oeq` / `olt` 等）；NaN-vs-anything 比较返 false。
     - **布尔 and/or/xor/not**：`and i1` / `or i1` / `xor i1` / `xor true, x`。这些是非短路；短路 `&&` / `||` 由 if-else 控制流处理。
     - **`==` / `!=` 对 ref types**：identity compare（pointer equality）。当前是否对 `String == String` 等 case 走 `scoop_string_equals`？检查 `mir_body/op.rs` line 197-220 区域；如果是，记下"`String` 的 `==` 在 P8 期间继续走 method dispatch 而**不是** `int_eq` LLVM 指令"。
     - **`==` / `!=` 对 `Bool` / `Char` / `Int` / `Float`**：直接 `icmp eq` / `fcmp oeq`。
  2. 对每个边界值行为写一行短测试 fixture 或 inline expression（不必创建 fixture 文件，文档里给可复现的代码片段即可）。
  3. 在文档末尾加"P8 实现守则"：
     - method intrinsic lowering 必须在所有列出的边界值上产出与上述完全相同的 LLVM IR / 运行结果。
     - 任何与基线不一致的行为变更必须独立 PR、单独评审、不在 P8 内顺手做。
- 必须遵从的约束：
  - 文档必须 commit 到 git（不像 P0-T01 的 baseline 清单可以在 `target/`）—— 这是设计决策，不是临时数据。
  - 文档完成前**不**开工 P8-T02 及之后的任务。
- 验证：
  1. `cat docs/reshape-baseline/operator-behavioral-baseline.md` —— 文档存在且覆盖上述每个 op 类别。
  2. 至少 5 条边界值（`Int.MIN_VALUE` 取负、`1.0 / 0.0`、`NaN < 1.0`、`Int.MIN_VALUE / -1`、shl amount = bit_width）有可执行验证片段。
- 完成条件：
  - 文档可作为 P8-T02 ~ P8-T05 的仲裁依据。
- 依赖：P7-T03。

完成记录（2026-05-17）：

- 改动范围：新增 `docs/reshape-baseline/operator-behavioral-baseline.md`，覆盖 `mir_body/op.rs` 现有一元/二元 operator lowering；同步更新 `TODO.md` 索引与本条任务标题。
- 核心决策：baseline 以当前实现为准，明确记录整型算术 wrap、除零和 `Int.MIN_VALUE / -1` 的 LLVM UB、移位计数 mask、signed/unsigned compare predicate、浮点无 fast-math 的 ordered predicate、`fcmp une` 的 NaN `!=` 行为、Bool 短路与非短路路径差异，以及 `String == String` 继续走 runtime 内容相等而非 pointer identity。
- 验证结果：`cat docs/reshape-baseline/operator-behavioral-baseline.md` 通过，文档存在并覆盖 P8-T01 要求的每个 op 类别；文档内包含 `Int.MIN_VALUE` 取负、`1.0 / 0.0`、`NaN < 1.0`、`Int.MIN_VALUE / -1`、`shl amount = bit_width` 等边界值验证片段；`git diff --check` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
- 与 `PLAN.md` 闭合：完成 P8 operator method 化前的行为仲裁文档；阶段级计划和依赖未变化，未修改 `PLAN.md`。
- 暂时性 failing fixture：无；本任务仅新增文档，不引入 fixture 或代码路径变更。

### [DONE] P8-T02：编译器 method-level intrinsic 表扩展

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P8 任务 T8-2 + entry key 表
  - `crates/scoopc/src/intrinsics.rs::{NamedIntrinsicAuditEntry, named_intrinsic_audit_entries}`（line 80-256）
  - `crates/scoopc/src/llvm/codegen/intrinsics/named.rs::{lower_array_size, lower_array_get, lower_array_set, lower_array_data_ptr}`（已有 4 条 entry 作为模板）
  - P8-T01 的 behavioral baseline
- 目标：
  - 在 method-level intrinsic 表新增 ~80 条 entry，覆盖整型 / 浮点 / 布尔 / Char 的算术 / 位 / 比较 / 逻辑 / 一元运算的 IR-direct lowering。
  - 每条 entry 的 `lower_*` 函数产出对应 LLVM 指令；行为与 P8-T01 baseline 逐位一致。
- 当前实现入口：
  - `crates/scoopc/src/intrinsics.rs::named_intrinsic_audit_entries`：现有 4 条 array entry 注册位置（line 94-117）—— 新 entry 在此追加
  - `crates/scoopc/src/llvm/codegen/intrinsics/named.rs`：现有 4 个 `lower_array_*` 函数（line 50-65 附近）—— 新 `lower_*` 函数在此追加
- 必须实现的内容：
  1. 在 `intrinsics.rs::named_intrinsic_audit_entries` 追加完整 entry 集合：
     - 整型（接受 `Int` / `UInt` / `Int8/16/32/64` / `UInt8/16/32/64` 共 10 种 receiver）：
       `int_plus / int_minus / int_times / int_div / int_rem / int_unary_minus / int_inc / int_dec / int_and / int_or / int_xor / int_inv / int_shl / int_shr / int_ushr / int_lt / int_le / int_gt / int_ge / int_eq / int_ne / int_compare_to`
       —— 这一组 22 条 entry 由 receiver 类型的 signed/unsigned 决定 lowering 选择（`sdiv` vs `udiv`、`ashr` vs `lshr`、`slt` vs `ult` 等）。**不**为每种 receiver type 重复 22 条—— entry 表统一这 22 个 name，lowering 函数内通过 receiver type info 分流。
     - 浮点（接受 `Float32` / `Float64`）：
       `float_plus / float_minus / float_times / float_div / float_rem / float_unary_minus / float_lt / float_le / float_gt / float_ge / float_eq / float_ne / float_compare_to / float_abs / float_is_nan / float_is_infinite / float_hash`
       —— 17 条 entry；lowering 内按 receiver f32/f64 分流。
     - 布尔：
       `bool_and / bool_or / bool_xor / bool_not`
       —— 4 条 entry。
     - Char：
       `char_to_int / char_hash / char_compare_to / char_equals / char_plus_int / char_minus_int / char_minus_char`
       —— 7 条 entry。`char_to_int` / `char_hash` 之前已有（在 sysroot core 现状中作为 intrinsic top-level fun）；P8-T03 把它们改为 method 形式后，其 entry name 仍按上述命名。
  2. 在 `crates/scoopc/src/intrinsics.rs::fallback_named_intrinsic_entry_name_for_fqn` 中加 FQN 映射：
     - `scoop.core.Int.plus` → `int_plus`
     - `scoop.core.Int.div` → `int_div`（lowering 函数内按 receiver 分 sdiv/udiv）
     - 同形扩展整型 22 条、浮点 17 条、布尔 4 条、Char 7 条
  3. 在 `crates/scoopc/src/llvm/codegen/intrinsics/named.rs` 实现对应 lower 函数：
     - `lower_int_plus`：args[0] 是 receiver、args[1] 是 RHS；都是 IntValue → `build_int_add`。
     - `lower_int_div`：检查 receiver type 的 signed flag → `build_int_signed_div` / `build_int_unsigned_div`。
     - `lower_int_shr`：检查 signed flag → `ashr` / `lshr`（注意：sysroot method `Int.shr` 等不区分 signed receiver；参考 Kotlin，`Int.shr` 是 ashr，`Int.ushr` 是 lshr。所以 entry 表实际是 `int_shr` = ashr / `int_ushr` = lshr，receiver 必为 signed type）。**实施时**确认 sysroot side `UInt.shr` / `UInt.ushr` 的语义—— Kotlin `UInt` 没有 `shr` 而是 `shr`/`ushr` 都是 lshr —— 决策记入完成记录。
     - `lower_float_div`：`build_float_div`。
     - `lower_float_eq`：`build_float_compare(FloatPredicate::OEQ, ...)`（NaN-aware）。
     - `lower_int_compare_to`：emit 三向比较（`slt → -1` / `eq → 0` / 否则 `1`），用 `select` chain 或 `phi`。
     - `lower_bool_and`：`build_and`（i1 类型）；非短路。
     - `lower_char_plus_int`：Char 是 i32 codepoint；`add i32` 后**不**做 codepoint 范围 clamp（保持当前行为；参考 P8-T01 baseline 中 Char 算术的边界）。
     - `lower_char_minus_char`：`sub i32` 得到差值（Int）。
  4. 行为对齐 P8-T01 baseline：每个 lowering 在写完后跑 baseline 中列出的边界值测试片段，确认输出一致。
  5. owner 测试（在 `crates/scoopc/src/llvm/codegen/intrinsics/named.rs` 同目录或 `crates/scoopc/src/llvm/tests/`）：
     - `int_plus_emits_add_inst`：snapshot test
     - `int_div_signed_vs_unsigned_diverges`：Int 与 UInt 走不同 IR
     - `float_eq_emits_oeq_predicate`
     - `int_compare_to_three_way_select`
     - `bool_and_emits_and_i1_not_select`：确认非短路（不生成 if-else）
- 必须遵从的约束：
  - 不修改任何现有 entry 的行为（`array_size/get/set/data_ptr`）。
  - 不在本任务实施 sysroot 声明（P8-T03）或 HIR 改写（P8-T04）—— 本任务只补 entry 表，跑不通 fixture 是预期的（无 caller）。
- 验证：
  1. `cargo build` —— 编译通过。
  2. `cargo test -p scoopc named_intrinsic -- --nocapture`
- 完成条件：
  - 全部新 entry 注册并实现 lowering；P8-T01 baseline 中列出的所有边界值在 unit test 形式下产出预期 IR。
- 依赖：P8-T01。

完成记录（2026-05-17）：

- 改动范围：扩展 `crates/scoopc/src/intrinsics.rs` 的 named intrinsic audit entry 表与 scalar method FQN fallback；扩展 `crates/scoopc/src/llvm/codegen/intrinsics/named.rs` 的 IR rule 表和 lowering；新增 `crates/scoopc/src/llvm/tests/named_intrinsic.rs` owner 测试并接入 `llvm/tests/mod.rs`；同步更新 `TODO.md` 索引与本条任务标题。
- 核心决策：新增 P8-T02 列出的 22 个 integer、17 个 float、4 个 bool、7 个 Char entry，全部保持 `IrEmission`；integer lowering 复用 P8-T01 baseline 的 wrap、signed/unsigned div/rem、shift count mask、signed/unsigned compare 规则；`int_shr` 按 receiver signedness 选择 `ashr/lshr`（因此 `UInt.shr` 为 `lshr`），`int_ushr` 始终为 `lshr`；float `!=` 使用 baseline 的 `fcmp une`；`compareTo` 用 `lt -> -1 / eq -> 0 / otherwise -> 1` 的 select chain；Char 算术按 i32 codepoint 运算且不做范围 clamp。
- 验证结果：`cargo test -p scoopc named_intrinsic -- --nocapture` 通过；`cargo build` 通过；`cargo clippy --all-targets -- -D warnings` 通过；`cargo test --all --all-targets` 通过。首次全量测试命令因工具 120s 总超时在 `gc_immix_try_minor_deadline` 附近被中止；随后该单测单独复跑 0.06s 通过，并用更长超时复跑全量测试通过。
- 与 `PLAN.md` 闭合：完成 P8 T8-2 的 method-level scalar intrinsic 表与 IR-direct lowering，为 P8-T03 sysroot method 声明和 P8-T04 operator 改写提供 entry surface；阶段级计划、依赖与完成标准未变化，未修改 `PLAN.md`。
- 暂时性 failing fixture：无；本任务不启用新的 sysroot caller 或 operator 改写路径，未新增 failing fixture。

### [DONE] P8-T03：sysroot 标量 type body 内补 `@Intrinsic("...")` method 声明

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P8 任务 T8-3 + Kotlin 命名约定
  - `sysroot/core.scoop` 中 `@Intrinsic struct Int / UInt / Char / Float32 / Float64 / Bool` body（line 44-73、440-456）
- 目标：
  - 在 sysroot 标量 type body 内加 method 声明（无 body，仅 `@Intrinsic("...")` 标记）。
  - method 名按 Kotlin 约定：`plus/minus/times/div/rem/unaryMinus/unaryPlus/inc/dec/and/or/xor/inv/shl/shr/ushr/compareTo/equals` 等。
- 当前实现入口：
  - `sysroot/core.scoop`：现有 6 个标量 type body（Int/UInt/Char/Float32/Float64/Bool）
  - P8-T02 引入的 entry name 表
- 必须实现的内容：
  1. **整型**（在 `Int` / `UInt` / `Int8/16/32/64` / `UInt8/16/32/64` 各 body 内）。以 `Int` 为例：
     ```
     @Intrinsic
     struct Int : Hashable, ToString {
         override fun toString(): String { return __scoop_int_to_string(this) }

         @Intrinsic("int_plus")
         fun plus(other: Int): Int

         @Intrinsic("int_minus")
         fun minus(other: Int): Int

         @Intrinsic("int_times")
         fun times(other: Int): Int

         @Intrinsic("int_div")
         fun div(other: Int): Int

         @Intrinsic("int_rem")
         fun rem(other: Int): Int

         @Intrinsic("int_unary_minus")
         fun unaryMinus(): Int

         @Intrinsic("int_unary_plus")
         fun unaryPlus(): Int

         @Intrinsic("int_inc")
         fun inc(): Int

         @Intrinsic("int_dec")
         fun dec(): Int

         @Intrinsic("int_and")
         fun and(other: Int): Int

         @Intrinsic("int_or")
         fun or(other: Int): Int

         @Intrinsic("int_xor")
         fun xor(other: Int): Int

         @Intrinsic("int_inv")
         fun inv(): Int

         @Intrinsic("int_shl")
         fun shl(amount: Int): Int

         @Intrinsic("int_shr")
         fun shr(amount: Int): Int

         @Intrinsic("int_ushr")
         fun ushr(amount: Int): Int

         @Intrinsic("int_compare_to")
         fun compareTo(other: Int): Int

         @Intrinsic("int_eq")
         override fun equals(other: Int): Bool   // 注意：override 要求 ToString/Hashable/Any 中有 equals 接口；如不存在，去掉 override

         override fun hash(): Int { /* 现有 body */ }
     }
     ```
     `UInt` 同形（同一组 entry name，但 receiver 是 UInt → P8-T02 lowering 会按 receiver type 分流到 `udiv` / `urem` / `lshr` 等）。
  2. **浮点** `Float32` / `Float64`：参照 Kotlin 命名 + P8-T02 entry list；`abs/isNaN/isInfinite` 已经在现 sysroot 中（line 254-268）—— 把它们从 top-level intrinsic 函数迁到 type body method，保持 entry name `float_abs/is_nan/is_infinite`（注意：现 entry 是 `Float64.abs` / `Float32.abs` 分两条—— P8-T02 表里是统一 `float_abs`，lowering 内按 receiver f32/f64 分；这一改动需要在 P8-T02 实施时确认形态）。
  3. **布尔** `Bool`：
     ```
     @Intrinsic
     struct Bool : Hashable, ToString {
         override fun toString(): String { /* 现有 body */ }

         @Intrinsic("bool_and")
         fun and(other: Bool): Bool

         @Intrinsic("bool_or")
         fun or(other: Bool): Bool

         @Intrinsic("bool_xor")
         fun xor(other: Bool): Bool

         @Intrinsic("bool_not")
         fun not(): Bool
     }
     ```
  4. **Char**：
     ```
     @Intrinsic
     struct Char : Hashable, ToString {
         override fun toString(): String { return __scoop_char_to_string(this) }

         @Intrinsic("char_to_int")
         fun toInt(): Int

         @Intrinsic("char_hash")
         override fun hash(): Int

         @Intrinsic("char_compare_to")
         fun compareTo(other: Char): Int

         @Intrinsic("char_equals")
         fun equals(other: Char): Bool

         @Intrinsic("char_plus_int")
         fun plus(offset: Int): Char

         @Intrinsic("char_minus_int")
         fun minus(offset: Int): Char

         @Intrinsic("char_minus_char")
         fun minus(other: Char): Int
     }
     ```
     注意：`Char.minus` 有两个 overload（`minus(Int)` 返 Char、`minus(Char)` 返 Int）—— 当前 Scoop 是否支持顶层同名 overload？前一轮 PLAN-managed-abi 中 prelude.scoop 有"LLVM codegen 的顶层函数仍不支持同名 overload"的注释。如果 method-level overload 已在前一轮 P4-T01 系列任务中开通，沿用；否则需要临时改名（如 `Char.minusChar(other: Char): Int`）并在完成记录里标注待 typecheck 收口后回名。
  5. **删除现有 top-level intrinsic 函数**：`sysroot/core.scoop` line 247-270 一组 `@Intrinsic fun Char.toInt()` 等顶层 extension function 形态，迁入 body method 后这些 top-level 声明删除。
  6. owner 测试：
     - `tests/fixtures/run-pass/scalar_method_intrinsic_basic.scoop`：
       ```
       fun main(): Int {
           val a: Int = 5
           val b: Int = 3
           require(a.plus(b) == 8)
           require(a.minus(b) == 2)
           require(a.times(b) == 15)
           require(a.div(b) == 1)
           require(a.rem(b) == 2)
           // ... 各 op 一条断言
           return 0
       }
       ```
     - 注意此时 `a + b` 仍走旧 codegen 路径（P8-T04 才切换）；本测试通过 method 形式调用直接验证 method intrinsic 在 P8-T03 后已可工作。
- 必须遵从的约束：
  - 不在本任务删除 `mir_body/op.rs`（P8-T05）或改写 `a + b` 为 method call（P8-T04）。
  - method 命名严格 Kotlin 风格——这一约束在 PLAN.md §3.3 (a) 已锁定。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/scalar_method_intrinsic_basic.scoop`
  2. `cargo run -p scoop -- test`（全量 baseline）—— 此时 `a + b` 仍走旧路径，无回退。
- 完成条件：
  - 用户可以写 `a.plus(b)` 等 method 形式并通过 method intrinsic 直接 lowering 到对应 LLVM 指令。
- 依赖：P8-T02。

完成记录（2026-05-17）：

- 改动范围：在 `sysroot/core.scoop` 的 Bool/Char/Float32/Float64/Int/UInt/Int8/16/32/64/UInt8/16/32/64 type body 中补充标量 method-level intrinsic 声明；删除已迁移的 Char/Float 顶层 numeric scalar extension intrinsic 声明；同步补齐 build fixture 的 sysroot overlay 中 `Int.hash` 声明；新增 `tests/fixtures/run-pass/scalar_method_intrinsic_basic.scoop`；同步更新 `TODO.md` 索引与本条任务标题。
- 核心决策：补齐 P8-T03 直接需要但 P8-T02 未注册的 `int_unary_plus` / `float_unary_plus`，并把既有 `Int.hash` 收口为 `int_hash` named IR intrinsic，避免 `Int.hash()` 继续走 retained member-call 后门；Char/Float/Int 的已迁移 helper 不再在 typecheck/HIR 保留旧特殊路径，统一走普通 member direct call + named intrinsic fallback；`Float*.toInt()` 暂保留 legacy method intrinsic lowering，因为 P8-T02 未定义 `float_to_int` named entry。
- 验证结果：`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/scalar_method_intrinsic_basic.scoop` 通过；`cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/literal_direct_call_float_only_is_error.scoop` 通过；`cargo test -p scoopc named_intrinsic -- --nocapture` 通过；`cargo clippy --all-targets -- -D warnings` 通过。
- 全量 baseline：`cargo run -p scoop -- test` 未完全通过，结果为 1335/1342 targets passed、1372 checks passed、7 targets failed。失败项均不在本任务新增的 scalar method fixture 内，已记录到本完成记录并由后续 `P13-T04` 最终 fixture 收尾兜底处理：`run-pass/mutable_array_ops_basic.scoop`、`runtime_gc/extern_enter_native_gc_arg_spill_reload.scoop`、`runtime_gc/extern_enter_native_roots_gc.scoop`、`runtime_gc/funptr_enter_native_roots_gc.scoop`、`runtime_gc/gc_handle_roundtrip.scoop`、`runtime_gc/gc_move_stackmap_heap_fixup.scoop`、`run_pass_cone/cross_file_ctor_named_default_basic`。
- 与 `PLAN.md` 闭合：完成 P8 T8-3 的 sysroot scalar method surface，为 P8-T04 operator desugar 提供可解析/可 lowering 的 method declarations；阶段级计划和依赖未变化，未修改 `PLAN.md`。
- 暂时性 failing fixture：见“全量 baseline”条目；本任务新增 fixture 已通过。

### P8-T04：HIR / typecheck——binary / unary operator 改写为 method call

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P8 任务 T8-4
  - `crates/scoopc/src/hir/lower/expr/members.rs::lower_binary_expr_type`（line 1402-1480）
  - `crates/scoopc/src/hir/lower/expr/`（HIR `lower_expr` 中 BinaryExpr / UnaryExpr 分支）
  - `crates/scoopc/src/ast/mod.rs::BinaryOp` / `UnaryOp` enum 定义
- 目标：
  - 把 `a + b` / `-a` / `a < b` / `a == b` / `!a` 在 HIR lowering 阶段改写为 `a.plus(b)` / `a.unaryMinus()` / `a.compareTo(b) < 0` / `a.equals(b)` / `a.not()` method call。
  - 短路 `&&` / `||` 保持现有 if-else lowering，**不**走 `Bool.and/or` method（保持短路语义）。
  - `==` / `!=` 对 ref types：保持当前行为（pointer identity 或调 `String.equals` 等 ref-type method body）—— 不改。
- 当前实现入口：
  - HIR lowering 中 binary expr 的 lowering 入口（grep `BinaryOp::Add` 在 `hir/lower/expr/`，应找到 `lower_expr` 内 BinaryExpr 分支）
  - typecheck 中 binary / unary 的类型检查与方法解析（grep `lower_binary_expr_type`）
- 必须实现的内容：
  1. 在 HIR lowering 阶段 `lower_expr::BinaryExpr(op, lhs, rhs)` 分支中：
     - 对 `op == Add`，emit `lhs.plus(rhs)` method call HIR 节点
     - `Sub` → `minus`、`Mul` → `times`、`Div` → `div`、`Rem` → `rem`
     - `BitAnd` → `and`、`BitOr` → `or`、`BitXor` → `xor`
     - `Shl` → `shl`、`Shr` → `shr`（注意 LHS receiver type 决定 ashr/lshr 由 method intrinsic lowering 处理；HIR 阶段不区分）
     - `Lt/Le/Gt/Ge`：emit `lhs.compareTo(rhs).<lt|le|gt|ge>(0)` —— 即 compareTo 后再链一个 Int 比较；或者**直接** emit `lhs.<lt|le|gt|ge>(rhs)` 走整型比较 method（P8-T02 注册的 `int_lt` 等 entry）。倾向第二种——更直接、IR 更简洁。
     - `Eq/Ne`：emit `lhs.equals(rhs)` 或 `lhs.equals(rhs).not()`（对 scalar），ref types 保持现有 dispatch（如 `String == String` 调 `String.equals` body method）。
     - `LogAnd`（短路 `&&`）/ `LogOr`（短路 `||`）：保持现有 if-else lowering。
     - `RangeInclusive`（`..`）/ `Elvis`（`?:`）：不动，保持现有 desugar。
  2. 在 HIR lowering 阶段 `lower_expr::UnaryExpr(op, operand)` 分支中：
     - `Neg` → `operand.unaryMinus()`
     - `Pos` → `operand.unaryPlus()`
     - `Not`：对 Bool emit `operand.not()`，对 Int / 其他整型 emit `operand.inv()`（如果 `!` 在 Scoop 中允许用于位反转；若仅 Bool，那 `inv` 由 `~` operator 触发；按 Scoop spec 实际语法）
  3. typecheck：
     - method dispatch 走 P4-T01l1 / l2 / l3 已落地的 builtin scalar receiver 路径——`int_lt` 等 entry 通过 `fallback_named_intrinsic_entry_name_for_fqn` 解析到 `scoop.core.Int.lt` FQN。
     - 类型推断保持现状：`a + b` 推 LHS/RHS 同类型 Int → 结果 Int（`lower_binary_expr_type` 现有逻辑）。
  4. 注意：从 HIR 角度看，desugar 后 `a + b` 不再是 BinaryExpr 节点，而是 MethodCall 节点。`mir_body/op.rs` 在 P8-T04 之后将看不到这些 BinaryOp。但 P8-T05 才删 `op.rs`。
  5. owner 测试：
     - `binary_op_lowers_to_method_call`：HIR snapshot test 验证 `a + b` 的 lowered HIR 含 `Int.plus` method call 节点。
     - `short_circuit_logical_and_does_not_lower_to_bool_and`：`a && b` 的 lowered HIR 仍是 if-else 控制流，不含 `Bool.and` method call。
     - `comparison_lowers_to_method_call`：`a < b` 的 lowered HIR 含 `Int.lt` method call。
     - `unary_minus_lowers_to_method`：`-a` 的 lowered HIR 含 `Int.unaryMinus` method call。
- 必须遵从的约束：
  - 短路 `&&` / `||` 必须保持短路语义—— 不允许 desugar 为 `Bool.and/or` method（那是非短路）。
  - ref types 的 `==` / `!=` 走现有路径—— 不强行改 `Int.equals` 形态（这是 scalar），ref types 用 `String.equals` 等 body method。
  - desugar 形态对所有标量类型一致（不为 `Int` vs `Float` 写两份 desugar 逻辑）—— receiver type 决定走哪个 method 由 typecheck 自然解析。
- 验证：
  1. `cargo test -p scoopc binary_op_lowers -- --nocapture`
  2. `cargo run -p scoop -- test`（全量 baseline）—— **预期**：算术相关 fixture 的 IR snapshot 大量变化（call instruction 包装一层），但运行结果与 P8-T01 baseline 完全一致。
  3. 抽样：选 5 个含算术的 baseline fixture，单跑确认 stdout 不变。
- 完成条件：
  - HIR 中所有 BinaryExpr / UnaryExpr（除短路逻辑、range、elvis）已 desugar 为 method call。
- 依赖：P8-T03。

### P8-T05：删除 `mir_body/op.rs` 按 `ast::BinaryOp` 直接 codegen 路径

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P8 任务 T8-5
  - `crates/scoopc/src/llvm/codegen/mir_body/op.rs`（408 行，整文件主要是 `codegen_mir_unary` / `codegen_mir_binary`）
- 目标：
  - 删除 `mir_body/op.rs` 中按 `ast::BinaryOp` 直接 emit LLVM 指令的路径。
  - P8-T04 后该路径已无 caller —— 本任务把死代码物理删除。
- 当前实现入口：
  - `crates/scoopc/src/llvm/codegen/mir_body/op.rs`
  - `crates/scoopc/src/llvm/codegen/mir_body/mod.rs`（如有的 dispatch 把 BinaryExpr / UnaryExpr 路由到 `codegen_mir_binary` / `codegen_mir_unary`）
- 必须实现的内容：
  1. 验证 `codegen_mir_binary` / `codegen_mir_unary` 已无 caller：
     - `grep -rn "codegen_mir_binary\|codegen_mir_unary" crates/scoopc/src/`—— 仅命中 `op.rs` 自身；如有其它命中说明 P8-T04 没改完，回去补。
  2. 删除 `crates/scoopc/src/llvm/codegen/mir_body/op.rs` 整文件（或仅清空函数；按 file 是否含其它有用 helper 决定）。
  3. 更新 `crates/scoopc/src/llvm/codegen/mir_body/mod.rs` 中的 `mod op;` 声明（如不再需要则删除）。
  4. 检查 MIR lowering 中 BinaryExpr / UnaryExpr 节点是否还会被产出：
     - 如果 P8-T04 在 HIR 阶段已经 desugar，MIR 里不应再有 BinaryExpr 节点；那 MIR 的 BinaryExpr enum variant 也可以删（在 `crates/scoopc/src/mir/mod.rs` 或对应位置）。
     - 如 MIR 仍保留 BinaryExpr 形态作为 IR 输入（虽无 producer），删除 enum variant。
  5. 删除 owner 测试中针对 `codegen_mir_binary` 的 unit test（如有）。
- 必须遵从的约束：
  - 仅在 P8-T04 完全切换且无 BinaryExpr/UnaryExpr 流入 `mir_body/op.rs` 后开工。
  - 不为"防御性兼容"保留任何 fallback 路径。
- 验证：
  1. `cargo build` —— 编译通过。
  2. `grep -rn "codegen_mir_binary\|codegen_mir_unary\|ast::BinaryOp::Add" crates/scoopc/src/llvm/`—— 完全无命中。
  3. `cargo run -p scoop -- test`（全量 baseline）—— 无回退。
- 完成条件：
  - LLVM codegen 中不再有按 `ast::BinaryOp` 的直接 dispatch。
- 依赖：P8-T04。

### P8-T06：算术 fixture 矩阵 + 边界值回归

- 参考：
  - [`PLAN.md`](./PLAN.md) §9 / P8 任务 T8-6 / T8-7
  - P8-T01 baseline 短文
- 目标：
  - 写一组系统性的算术 / 比较 / 位 / 逻辑 fixture，每种类型 × 每种 op × 每个边界值组合各一条。
  - 这一组 fixture 是 P8 后所有未来变更（包括 spec 演进）的 regression bedrock。
- 当前实现入口：
  - `tests/fixtures/run-pass/`：P8-T06 添加新 fixture
  - P8-T01 的 `docs/reshape-baseline/operator-behavioral-baseline.md` 提供 ground truth
- 必须实现的内容：
  1. 在 `tests/fixtures/run-pass/` 下新建一组 fixture（建议命名 `operator_<type>_<category>.scoop`）：
     - `operator_int_arithmetic.scoop`：Int 的 plus/minus/times/div/rem/unaryMinus，含正常值 + `Int.MIN_VALUE` 取负 + `Int.MIN_VALUE / -1` + `0 / 1` + `1 / 0`（如果当前 baseline 是 trap，则 fixture 必须用 typecheck 形式而不是 run-pass）。
     - `operator_int_bitwise.scoop`：and/or/xor/inv + shl/shr/ushr，含 amount = 0 / amount = bit_width - 1 / amount = bit_width（注意 P8-T01 baseline 中此情形的处理）。
     - `operator_int_compare.scoop`：lt/le/gt/ge/eq/ne/compareTo，含正负边界 + 等值 + 上下界。
     - `operator_uint_div_rem.scoop`：UInt 走 udiv/urem，覆盖 max value 边界。
     - `operator_float64_arithmetic.scoop`：plus/minus/times/div/rem，含 NaN 传播 + inf 算术 + 0.0 / 0.0 + 1.0 / 0.0 + -inf + inf。
     - `operator_float64_compare.scoop`：含 `NaN < 1.0` / `NaN == NaN` / `+0.0 == -0.0`。
     - `operator_float32_basic.scoop`：与 float64 同形，覆盖 f32 精度边界。
     - `operator_bool_logic.scoop`：and/or/xor/not 的真值表（4 个 case 各覆盖）+ 短路 `&&` / `||` 不调用 RHS 的副作用验证。
     - `operator_char_arithmetic.scoop`：plus(Int) / minus(Int) / minus(Char) / compareTo / equals。
  2. 每个 fixture 的 EXPECTED 输出必须直接从 P8-T01 baseline 短文 copy（保持 ground truth 一致性）。
  3. 在 `tests/fixtures/typecheck/` 下补一条 typecheck-fail fixture：`operator_short_circuit_does_not_call_method.scoop`—— 验证短路 `&&` / `||` 在 lowered HIR 中**不**含 `Bool.and` / `Bool.or` method call（snapshot 形式）。
- 必须遵从的约束：
  - fixture 必须**通过 method 形式 + operator 形式两种**调用都覆盖到——确保 desugar 路径与直接 method 调用产出相同结果。
  - fixture 期望 stdout 必须严格匹配；任何与 baseline 短文不一致的输出表明 lowering 行为变更，须先回写文档。
- 验证：
  1. `cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/operator_*.scoop`
  2. `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck/operator_short_circuit_does_not_call_method.scoop`
  3. `cargo run -p scoop -- test`（全量 baseline）—— 无回退。
- 完成条件：
  - operator 行为有完整 fixture 覆盖；P8 整体收口。
- 依赖：P8-T05。
