# TODO（Scoop：当前审计剩余 issue 收口）

> 生成时间：2026-04-18  
> 历史归档：`TODO-4.md` / `PLAN-4.md`  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本轮以 `ISSUES.md` 当前仍未关闭的条目为主线；若执行中发现新的前置 blocker，需先插入到依赖它的任务之前。

## 全局约束

- `TODO.md` 中已标记 `[DONE]` 的条目只作历史归档；即使 `ISSUES.md` 改写了问题描述，也只能通过新增未完成任务继续收口，不能回写这些已完成条目。
- 当前剩余实现顺序为：effect / continuation -> `Task` object model -> value semantics / or-pattern -> annotation / `inline` -> FFI / ABI -> const / comptime。
- 每个实现任务后必须立即做 review 任务；review 只审查生产代码与规范一致性，不以测试命名代替结论。
- 若某项实现改变公开语义，必须同步 `SCOOP_FULL_SPEC.md`；若涉及运行时合同，还要同步 `SCOOP_RUNTIME.md` 或相关 sysroot 文档。
- 本轮不设计 executor framework；所有与 executor、wakeup、queueing、work-stealing、spawn scheduling 相关内容一律留待后续。当前代码库里 premature / incomplete 的 executor implementation 也应作为待移除项对待，不能继续把它当作 `Task` core 的前提。

## T4001：泛型约束、参数化超类型与 star projection

### T4001 [DONE] 收口泛型约束、参数化超类型与 star projection 语义
- 范围：
  - `where` 子句支持带类型实参的 nominal bound。
  - type env 记录 direct supertypes 时保留 type args。
  - assignable / 上转规则支持参数化超类型。
  - `*` 不再简单退化为 `Any`，需要有真实 star projection 语义。
- 验收：
  - 覆盖 typecheck、assignable、lowering、必要的 run-pass / regression。
  - `ISSUES.md` 第 5 条收窄或关闭。
- 完成：
  - 已为 `TypeEnv` / `TypeLowering` / `assignable` / RTTI / LLVM codegen 补齐参数化超类型与 star projection 主线。
  - 已新增 6 条回归 fixture，覆盖参数化 where bound、参数化超类型上转与 `Array<*>` 读视图。
  - 已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`、定向 run-pass、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
- 依赖：无

### T4001R [DONE] Review：确认参数化超类型与 star projection 没有退回特判
- 重点：
  - 不允许只对个别 interface / collection 做旁路特判。
  - star projection 不能只是换个位置继续降成 `Any`。
- 验收：
  - review 结论明确写入提交说明或文档变更中。
- 完成：
  - 已复审 `TypeEnv -> TypeLowering -> assignable` 主线：参数化超类型通过 `direct_supertype_infos` 保留声明处 `TypeRef`，在实例化时统一 substitution 为 `concrete_direct_supertypes`，未引入 `Array` / `Collection` / 单个 interface 名称的旁路分支。
  - 已复审 `TypeKind::StarProjection` 主线：`*` 在 typecheck 内保留为独立类型节点，只在 ScoopIR 导出、RTTI 布局和 LLVM CG 类型映射边界读取 `read_ty`，没有在前端主线上提前退化成 `Any`。
  - 已验证 `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (326)`）与定向 run-pass `target/debug/scoop test --fixtures target/t4001r-fixtures/run-pass`（`fixtures: ok (2)`）。
- 依赖：T4001

## T4002：lambda 推断与 receiver lambda

### T4002 [DONE] 补齐 lambda expected-type 推断与 receiver lambda 基本语义
- 范围：
  - 放宽“无 expected type 就直接报错”的当前门禁。
  - 扩展 expected-type 向下传播，不再只停在 0/1/2 参数。
  - receiver lambda body 中补齐 `this` 注入语义。
- 已完成：
  - lambda expected-type 推断已从写死 0/1/2 参数推广到统一按函数签名处理，并保留“无参数列表 + 恰好 1 个普通参数”时的隐式 `it` 规则。
  - 无 expected type 的 lambda 现支持“显式参数类型”与零参数场景直接定型，不再一概拒绝。
  - receiver lambda 的隐式 `this` 已在 resolver / typecheck / HIR / LLVM closure codegen 主线上贯通，member access / method call 会按 receiver 实际类型 late resolve，并正确遮蔽外层 `this`。
- 验收：
  - 新增对应 typecheck / run-pass fixtures。
  - `ISSUES.md` 第 3 条收窄或关闭。
- 已验证：
  - `target/debug/scoop test --fixtures target/t4002-fixtures/infer`（`fixtures: ok (4)`）
  - `target/debug/scoop test --fixtures target/t4002-fixtures/run-pass`（`fixtures: ok (4)`）
  - `target/debug/scoop test --fixtures tests/fixtures/typecheck`（`fixtures: ok (326)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4001R

### T4002R [DONE] Review：确认 lambda 推断主线统一，不靠局部 call-shape 补丁
- 重点：
  - 不允许只为某个调用形态单独补推断。
  - receiver lambda 的 `this` 语义必须和普通 receiver function type 对齐。
- 完成：
  - 复审 direct call / member call / receiver-function-value call 的 lambda expected-type 传播路径后，确认主线仍统一走“先按签名收集候选，再仅对 lambda 实参进入 expected-context typecheck”的模式，没有重新回到 0/1/2 参数或单一 call-shape 的局部补丁。
  - 发现并修复了一个既有语义裂缝：当 receiver lambda 嵌套在已有 `this` 上下文中时，typecheck 会按 receiver 实际类型晚解析 `this.member(...)`，但 HIR lowering 仍沿用 resolver 的旧 `member.resolved` 与旧 `this` 绑定，导致 `receiver_lambda_this_shadows_outer_this` 实际输出错误的 `99`。
  - 已用两条统一主线收口该裂缝：一是新增 typecheck `typechecked_member_resolved` side table，让 lowering / codegen 读取成员最终决议；二是在 HIR lowering 进入 receiver lambda body 时维护当前隐式 `this` 绑定，并让内建字符串/标量方法保留为 member-call 形态交给后端 intrinsic 路径，而不是误改写成外层 class member 顶层调用。
- 已验证：
  - `target/debug/scoop test --fixtures target/t4002r-fixtures/infer`（`fixtures: ok (1)`）
  - `target/debug/scoop test --fixtures target/t4002r-fixtures/run-pass`（`fixtures: ok (4)`）
  - `target/debug/scoop test --fixtures tests/fixtures/typecheck`（`fixtures: ok (326)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4002

## T4003：调用语义早期门禁

### T4003 [DONE] 收口函数值 / funptr / constructor delegation 的调用语义差异（拆分执行）
- 说明：
  - 原任务同时跨越 `FunPtr<F>` receiver signature、顶层泛型函数值 / `callee<T>` 表示、以及 ctor delegation 的命名/默认参数绑定三套基础设施。
  - 为保证每轮只提交一个完整且可验证的切片，现拆分为 `T4003a -> T4003b -> T4003c` 顺序推进。
- 验收：
  - 子任务全部完成后，对应调用形态都有回归。
  - `ISSUES.md` 第 4 条收窄或关闭。
- 完成：
  - `T4003a` / `T4003b` / `T4003c` / `T4003R` 已按顺序收口函数值、`FunPtr` 与 ctor delegation 的统一调用语义。
  - 在推进 `T4004` 前额外插入的 `T4003S` / `T4003SR` / `T4003T` / `T4003TR` 两组前置 blocker 与 review 也已完成，后续顶层 pattern binding 可直接建立在统一的顶层 immutable value 与局部 destructuring 主线上继续推进。
- 依赖：T4002R

### T4003a [DONE] 打通 `FunPtr<F>` 的 receiver function type 调用语义
- 范围：
  - `FunPtr<T.(...) -> R>` 不再在类型 lowering 阶段被提前拒绝。
  - direct call `fp(receiver, ...)` 与 `FunPtr.invoke(...)` 在 receiver signature 下统一按“receiver 作为第 0 个实参”检查与执行。
  - 补充对应 unsafe run-pass / typecheck 回归。
- 验收：
  - `FunPtr` receiver signature 可通过 typecheck，并能在 unsafe context 中正确执行。
  - direct call 与 `.invoke(...)` 语义一致。
- 完成：
  - 已移除 `FunPtr<F>` 对 receiver function type 的 early gate，并让 funptr direct call / sysroot `invoke` 统一按“receiver 作为第 0 个显式实参”做类型检查与 LLVM indirect call。
  - 已为 `scoop.unsafe.FunPtr` 补充 receiver 形态的 `invoke` overload，使 `fp.invoke(receiver, ...)` 与命名实参路径可用。
  - 已新增 `unsafe_funptr_receiver_call_basic` run-pass 回归，覆盖 direct call、`.invoke(...)` 与 `.invoke(receiver = ..., a0 = ...)`。
- 已验证：
  - `cargo run -p scoop -- test --fixtures target/t4003a-fixtures/run-pass`（`fixtures: ok (3)`，含新增回归与既有 funptr 回归）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (326)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4002R

### T4003b [DONE] 支持顶层泛型函数值与 `callee<T>` 一等值传递
- 范围：
  - 顶层函数在值位置可形成函数值。
  - `callee<T>` 不再只允许作为 `Call` 的 callee 透明包装，而可作为一等值传递给 higher-order 调用。
  - 对应 HIR / monomorph / codegen 路径补齐。
- 验收：
  - 新增 higher-order / run-pass 回归，覆盖 `callee<T>` 赋值、传参与后续调用。
- 完成：
  - 已为 bare 顶层函数值与 `callee<T>` 建立 typecheck side table，并贯通 AST / parser / typecheck / HIR lowering，使顶层函数在值位置可稳定形成函数值。
  - typecheck 现支持根据 expected function type 反推泛型函数值 type args；higher-order 调用预收集实参时，会把 bare 顶层函数值候选先保留为占位 `Any`，避免在 expected-context 生效前过早报 `generic_type_arg_not_inferred`。
  - HIR lowering 现将 typecheck 选中的顶层函数值统一合成为零捕获 closure 包装，直接复用现有 function-value call / monomorph / codegen 主线，没有新增第三套运行时表示。
  - parser 已放宽 type-apply lookahead，`callee<T>` 现在既可继续直接调用，也可作为普通值表达式赋值、传参与返回。
- 已验证：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/top_level_generic_function_value_basic.scoop -o /tmp/t4003b.out`
  - `/tmp/t4003b.out` 输出依次为 `3`、`4`、`10`、`20`、`5`
  - `cargo run -p scoop -- test --fixtures target/t4003b-fixtures/run-pass`（`fixtures: ok (1)`）
  - `cargo run -p scoop -- test --fixtures target/t4003b-fixtures/typecheck`（`fixtures: ok (1)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (327)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4003a

### T4003c [DONE] 为函数值 / funptr / ctor delegation 收口命名实参与默认参数绑定
- 范围：
  - 函数值与 funptr 的命名实参支持边界不再靠早期硬拒绝分流。
  - `super(...)` / `this(...)` 构造器委托调用接入与普通调用一致的命名 / 默认参数绑定语义。
  - 调用参数重排与默认值补齐所需的 side table / lowering 元数据补齐。
- 验收：
  - 对应调用形态都有回归。
  - constructor delegation 不再只允许位置参数。
- 已完成：
  - typecheck 为 direct function-value call / direct `FunPtr` call 统一启用命名实参与参数重排，形参命名规则收口为 `receiver` / `a0` / `a1` / ...，并与 LLVM codegen 使用同一套映射。
  - class ctor / header super ctor / `this(...)` / `super(...)` 统一改为记录 typechecked ctor binding side table，HIR 与 LLVM 直接复用已选中的 ctor 目标与 `arg_mapping`，不再按 arity 重新猜。
  - ctor 参数默认值已进入 HIR / LLVM 主线，显式实参保持源码顺序求值，缺失形参再按绑定后的形参顺序补默认值。
  - effect state-machine 相关的 ctor call target 消费点已切到新的 `CtorCallInfo` 模型；无完整 typecheck 的 IR 测试入口也补上了基于 resolver call-shape 的保守 ctor fallback，避免 `emit_minimal_main_ir` 路径把 direct class ctor call 误掉进 enum variant ctor 分支。
  - 新增 run-pass 回归：
    - `tests/fixtures/run-pass/function_value_named_args_basic.scoop`
    - `tests/fixtures/run-pass/unsafe_funptr_direct_named_call_basic.scoop`
    - `tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`
- 已验证：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/function_value_named_args_basic.scoop -o /tmp/fv_named.out`
  - `cargo run -p scoop -- build tests/fixtures/run-pass/unsafe_funptr_direct_named_call_basic.scoop -o /tmp/fp_named.out`
  - `cargo run -p scoop -- build tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop -o /tmp/ctor_named.out`
  - `cargo run -p scoop -- test --fixtures /tmp/t4003c-fixtures`（`fixtures: ok (6)`）
  - `cargo run -p scoop -- test --fixtures /tmp/t4003c-typecheck`（`fixtures: ok (10)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (327)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4003b

### T4003R [DONE] Review：确认调用系统不再按 callee 形态分裂
- 重点：
  - direct call、member call、function-value call、funptr call 不能各自维护不同规则分支。
- 完成：
  - 复审 typecheck / HIR / LLVM 后确认存在一个既有裂缝：function value / funptr 已能在 codegen 侧按命名实参重排，但顶层 direct call、vtable member call 与 itable member call 仍各自要求纯位置实参，导致命名实参在 build 阶段落入 `named call arg` 一类后端错误；顶层泛型 direct call 的 monomorph FQN 解析也仍按位置索引读取实参，无法与命名实参共享同一套绑定语义。
  - 已将 LLVM 调用参数绑定收口为共享主线：`map_call_args_to_params_by_name` 负责统一映射 named/positional args，`codegen_bound_call_args` 负责按源码顺序求值后再按形参顺序归位。direct call、vtable、itable、function-value、funptr 现都复用该主线；`scoop.unsafe.invoke` 不再单独重排命名实参，而是直接复用 funptr callable-value binder。
  - 已把顶层泛型 direct call 的 monomorph FQN 解析切到同一套命名实参映射，避免 `pick(b = ..., a = ...)` 这类场景因按位置索引读取实参而漏掉 concrete type 绑定。
  - 已新增 3 条 run-pass 回归：
    - `tests/fixtures/run-pass/top_level_generic_named_args_basic.scoop`
    - `tests/fixtures/run-pass/member_call_virtual_named_args_basic.scoop`
    - `tests/fixtures/run-pass/member_call_interface_named_args_basic.scoop`
- 已验证：
  - `cargo run -p scoop -- test --fixtures /tmp/t4003r-run-pass`（`fixtures: ok (6)`，覆盖 direct/member/function-value/funptr 命名调用）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (327)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4003c

### T4003S [DONE] 收口普通顶层 `val` 的可执行读取语义
- 说明：
  - 在推进 `T4004` 前，发现普通顶层 `val` 的 build/runtime 主线本身仍未打通：`val x: Int = 41; fun main(): Int { return x + 1 }` 当前会在 LLVM codegen 阶段报 `top-level value ref`。
  - 顶层 pattern binding 会把多个 binder 暴露为普通顶层 immutable value；如果继续在“只有 `const val` / 静态 `var` 可读”的现状上实现 `T4004`，就会把新特性错误地绑死在旁路表示上，违反“与现有顶层 `val` 统一语义”的要求。
- 范围：
  - 非 `const` 顶层 `val` 需要具备稳定的 HIR / LLVM 表示与读取路径，不再落入 `top-level value ref` unsupported。
  - 顶层 immutable value 的初始化/引用语义需要统一，不能通过“把普通 `val` 偷偷当成 `const val` 重复内联”的方式过关。
  - 新增 lowering / run-pass 回归，覆盖普通顶层 `val` 被 `main` 和其它顶层 initializer 读取。
- 验收：
  - 上述最小 probe 可 build/run。
  - 为 `T4004` 提供可复用的顶层 immutable binder 表示。
- 已完成：
  - HIR lowering 现为命名的非 `const` 顶层 `val` 收集 `top_level_immutable_values` side table，和 `const val` / 静态 `var` 分离表示，为后续顶层 pattern binder 复用预留了统一入口。
  - LLVM codegen 现为普通顶层 immutable value 生成 module-local backing global、once guard 与按需定义的 init function；`codegen_var_ref` 读取时会先确保初始化，再加载结果，不再落入 `top-level value ref` unsupported。
  - 顶层 immutable value 的 reachability 扫描现会递归扫描其 initializer，effect state-machine 也已把这类读取收口为隐藏的一次性初始化边界，避免只在普通 codegen 主线上“偶然可用”。
  - 已新增回归：
    - `tests/fixtures/build/top_level_val_read_minimal_ok.scoop`
    - `tests/fixtures/run-pass/top_level_val_runtime_read_basic.scoop`
- 已验证：
  - `cargo run -p scoop -- build tests/fixtures/build/top_level_val_read_minimal_ok.scoop -o /tmp/top_level_val_read_minimal_ok.out`
  - `/tmp/top_level_val_read_minimal_ok.out`（退出码 `42`）
  - `cargo run -p scoop -- test --fixtures /tmp/t4003s-run-pass`（`fixtures: ok (1)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4003R

### T4003SR [DONE] Review：确认普通顶层 `val` 不再依赖 `const` / 静态 `var` 旁路
- 重点：
  - 不允许只让 `const val` 或 `@ThreadLocal/@Global var` 可执行，而普通顶层 `val` 仍报 `top-level value ref`。
  - 顶层 immutable value 的初始化次数与读取语义要有统一结论，不能为 `T4004` 额外再开第四套表示。
- 完成：
  - 已复审 HIR lowering / LLVM codegen / reachability / effect state-machine 主线：命名的非 `const` 顶层 `val` 统一经由 `top_level_immutable_values` side table、module-local backing global、once guard 与 init function 执行，不再借用 `const val` 或静态 `var` 表示，`T4004` 可直接复用这套 immutable binder 主线。
  - 复审中发现并修复一个既有语义裂缝：同线程递归初始化重入时，`scoop_once_begin` 会返回 `0` 以避免死锁，但原访问路径会继续读取尚未完成初始化的 backing global，导致 `val x: Int = x + 1` 一类程序错误地把零值读成合法结果。
  - LLVM 顶层 immutable value 访问现会在 init function 返回后再次检查 guard 状态；若 guard 仍未进入 `initialized`，则立即以退出码 `1` 终止，阻止递归初始化把未初始化值伪装成合法结果。
  - 已新增 `tests/fixtures/run-pass/top_level_val_recursive_init_is_error.scoop` 回归，覆盖“通过 helper 函数间接回读顶层 `val`”的递归初始化场景，避免只靠 AST 形状特判过关。
- 已验证：
  - `cargo run -p scoop -- test --fixtures /tmp/t4003sr-run-pass`（`fixtures: ok (2)`，覆盖正常读取与递归初始化失败）
  - `cargo run -p scoop -- build tests/fixtures/build/top_level_val_read_minimal_ok.scoop -o /tmp/top_level_val_read_minimal_ok.out`
  - `/tmp/top_level_val_read_minimal_ok.out`（退出码 `42`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4003S

### T4003T [DONE] 收口局部 `val` pattern binding 的可执行 lowering / codegen
- 说明：
  - 在尝试启动 `T4004` 时，最小 probe `fun main(): Int { val pair: (Int, Int) = (1, 2); val (a, b) = pair; return a + b }` 当前会在 build 阶段报 `scoop::llvm::unsupported_main_body: anonymous val binding`。
  - 这说明局部 `val` destructuring 目前只完成了 parser/typecheck，HIR `ValDecl` 仍把 pattern 绑定降成匿名声明，后端缺少统一的 binder lowering；若继续直接做顶层 pattern binding，只会引入额外的顶层专用 ad-hoc 路径，违反后续 review 对“顶层/局部复用同一套语义”的要求。
- 范围：
  - 让局部 `val` pattern binding 在 HIR / LLVM 主线上可执行，initializer 只求值一次，binder 读取复用统一的解构/投影语义。
  - tuple / struct / enum variant destructuring 与既有 `typecheck::val_pat` 规则对齐。
  - `var` destructuring 继续按 spec §4.2 报错，不新增例外分支。
- 验收：
  - 上述最小 probe 可 build/run。
  - 新增 lowering / run-pass 回归，覆盖 tuple / struct / enum variant binder 的读取。
  - 局部 pattern binding 不再落入 `anonymous val binding` unsupported。
- 已完成：
  - HIR lowering 现为局部 `val` pattern binding 生成 synthetic subject + 多条命名 `ValDecl`，initializer 只求值一次；tuple / struct / enum variant binder 统一走合成投影 / `when` 提取，不再把 pattern 绑定降成匿名局部。
  - block / stmt lowering 现支持“单条 AST 语句展开成多条 HIR 语句”，为局部 destructuring 主线提供统一承载，而不是额外开一条顶层/局部分叉的 ad-hoc 后端路径。
  - typecheck 现把局部 pattern binder 的推断类型写回 side table；`lower_val_decl` 也优先复用 initializer 的 typechecked type，避免 subject / tuple literal / enum-rich aggregate 在 codegen 侧退化成 `Any`。
  - HIR struct layout 收集现补齐 body-property 风格的 struct 字段（含泛型实例化路径），修复 `struct Point { val x: Int; val y: Int }` 这类 destructuring / field access 的 LLVM 字段投影失败。
  - 已新增回归：
    - `tests/fixtures/hir/local_val_destructuring_lowering.scoop`
    - `tests/fixtures/hir/local_val_destructuring_lowering.hir`
    - `tests/fixtures/run-pass/local_val_destructuring_tuple_struct_variant_basic.scoop`
    - `tests/fixtures/run-pass/local_val_destructuring_tuple_struct_variant_basic.stdout`
    - `tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`
    - `tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.stdout`
- 已验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/local_val_destructuring_tuple_struct_variant_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/hir`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4003SR

### T4003TR [DONE] Review：确认局部 destructuring 主线已可被顶层复用
- 重点：
  - 不接受把局部 pattern binding 仅靠“匿名 val + 特判读取”糊过去。
  - 顶层后续应能直接复用同一套 binder lowering / 投影语义。
- 完成：
  - 复审 `lower_local_pattern_val_stmt -> codegen_val_decl` 主线后，确认局部 destructuring 现已通过“合成 subject + 多条命名 binder `ValDecl`”落到现有局部绑定语义；LLVM `codegen_val_decl` 仍显式拒绝匿名 `ValDecl`，因此当前实现不存在“匿名 val + 特判读取”的旁路。
  - 复审 `synth_pattern_runtime_check_expr`、`synth_pattern_binding_init_expr` 与 `collect_pattern_binders` 后，确认 tuple / struct / enum variant 的投影与运行期校验已被抽成接受任意 subject `Expr` 的通用 helper；后续顶层实现只需补齐 binder 符号安装与 `top_level_immutable_values` once-init 承载，无需另开一套 destructuring / 投影语义。
  - 额外用临时 probe `makePair()` 复验 destructuring initializer 只求值一次（stdout 为 `7`、`42`），与 HIR 中的 synthetic subject 结构一致，没有发现重复求值裂缝。
- 已验证：
  - `cargo run -p scoop -- run /tmp/t4003tr_local_single_eval_probe.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/hir`（`fixtures: ok (16)`）
  - `cargo run -p scoop -- run tests/fixtures/run-pass/local_val_destructuring_tuple_struct_variant_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (327)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4003T

## T4004：顶层 `val` pattern binding

### T4004 [DONE] 打通顶层 `val` 的 pattern binding（拆分执行）
- 说明：
  - 复查 spec §4.2 / Appendix B.11 后确认 destructuring 仅适用于 `val`；`var` 不支持 destructuring patterns，因此原“顶层 `val` / `var`”表述收窄为“顶层 `val`”。
  - 顶层版本还同时跨越“binder 符号安装 / 类型收集”和“once-init lowering / codegen”两条主线；其中前者在落地时又拆出了“显式整体类型注解路径”和“initializer 驱动推断 + 跨文件可见性”两块。
  - 为避免单轮同时横跨 parser / resolver / typecheck / 跨文件 value table / lowering，现细化为 `T4004a1 -> T4004a2 -> T4004b -> T4004R` 顺序推进。
- 验收：
  - 子任务全部完成后，顶层 tuple / struct / enum destructuring 可跨文件引用并稳定执行。
  - `ISSUES.md` 第 6 条收窄或关闭。
- 完成：
  - `T4004a1` / `T4004a2` / `T4004b` / `T4004R` 已全部完成；顶层 tuple / struct / enum pattern `val` 现已同时打通 parser / resolver / typecheck / 跨文件值类型可见性 / HIR / LLVM once-init / state-machine hidden boundary 主线。
  - 复审阶段额外发现并修复了一个既有裂缝：当顶层 pattern binder 在 `handle` / `try` 的 state-machine 路径里触发隐藏 check `Raise.raise(RuntimeError.*)` 时，旧实现会因漏接 `TopLevelValueInitAccess` 的 hidden suspend 处理而把 guard 的 `initializing` 状态误判为递归初始化，直接 `exit(1)`；现已统一改为沿 active/inactive boundary 进入 handler dispatch。
- 依赖：T4003TR

### T4004a [DONE] 将顶层 `val` pattern binder 的静态接入拆分为“显式注解路径”与“推断 / 跨文件路径”
- 说明：
  - 继续细化后发现，原 `T4004a` 同时要求：
    - 顶层 parser / resolver 接入 pattern binder；
    - 顶层整体类型注解向 binder 类型分发；
    - 无整体类型注解时从 initializer 推断整体类型；
    - 让 binder 类型进入跨文件可见的 top-level value type 表。
  - 其中后两者已经直接碰到当前“顶层值类型表仍只覆盖当前文件”的通用缺口；因此先把“显式整体类型注解 + 同文件静态引用”独立成可交付切片，再在下一步补 initializer 推断与跨文件可见性。
- 完成：
  - 已将原 `T4004a` 细化为 `T4004a1 -> T4004a2`，后续 `T4004b` / `T4004R` 依赖顺延。
- 依赖：T4003TR

### T4004a1 [DONE] 打通顶层 `val` pattern binder 的 parser / resolver 索引与显式整体类型注解路径
- 范围：
  - 顶层 `val` 解析支持 tuple / struct / enum pattern，以及 `val <pattern>: Type = initializer` 形式的整体类型注解。
  - 顶层 pattern binder 安装到 value namespace / `top_level_types`，先覆盖同文件静态引用。
  - 顶层 `var` pattern binding 继续按 spec §4.2 拒绝，错误与局部规则对齐。
  - 无整体类型注解的顶层 pattern binding 暂继续报错，等待 `T4004a2`。
- 验收：
  - 新增 parse / typecheck 回归：同文件顶层 tuple / struct / enum binder 被后续顶层声明或函数体引用。
- 完成：
  - 顶层 `val` parser 现已支持 tuple / struct / enum pattern，并接受 `val <pattern>: Type = initializer` 形式的整体类型注解；顶层 `var` destructuring 继续在 parser 阶段按与局部路径一致的规则拒绝。
  - `ValBinding` 新增统一的 `bound_idents()` helper；resolver index 与 block scope 检查现都通过同一入口收集 binder，顶层 pattern binder 会被注入 value namespace，供同文件后续顶层声明与函数体解析。
  - `check_top_level_val_header` 现允许“带整体类型注解的顶层 pattern binding”，并继续对“无整体类型注解的顶层 pattern binding”报 `missing_type_annotation`，把 initializer 驱动推断明确留给 `T4004a2`。
  - `collect_top_level_value_types` 现会把顶层 pattern 的整体类型经 `val_pat::infer_val_pat_bindings` 分发到各 binder；顶层 initializer typecheck 也会把这些 binder 类型写回 side table，为后续 `T4004b` 复用。
  - 已新增回归：
    - parser 单测 `parse_top_level_val_destructuring_with_type_annotation`
    - parser 单测 `top_level_var_destructuring_is_rejected`
    - `tests/fixtures/typecheck/top_level_val_pattern_annotated_same_file_ok.scoop`
    - `tests/fixtures/typecheck/top_level_val_pattern_missing_type_is_error.scoop`（已在 `T4004a2` 中被推断通过回归替换）
- 已验证：
  - `cargo test -p scoopc top_level_`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (329)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4003TR

### T4004a2 [DONE] 为顶层 `val` pattern binder 补齐 initializer 驱动推断与跨文件类型可见性
- 范围：
  - 顶层 `val` pattern binding 的整体类型可来自 initializer 推断，不再强制显式整体类型注解。
  - 顶层 pattern binder 的类型进入跨文件可见的 top-level value type 表，供其它文件静态引用。
  - 新增多文件 typecheck 回归：顶层 tuple / struct / enum binder 被其它文件引用。
- 验收：
  - 顶层 tuple / struct / enum binder 的无注解写法与多文件静态引用均可通过 typecheck。
- 完成：
  - `check_top_level_val_header` 现已放宽为“仅普通顶层命名 `val/var` 继续强制显式类型注解”；顶层 `val` pattern binding 会把整体类型推断留给 initializer typecheck，不再在 header phase 早退报 `missing_type_annotation`。
  - `TypeEnv` 现保留编译单元文件 AST 视图；`collect_top_level_value_types` 也从“只扫当前文件显式注解”升级为“跨文件显式收集 + 无注解顶层 pattern binder 迭代推断”，从而把 tuple / struct / enum binder 类型写入跨文件可见的 top-level value type 表。
  - 顶层 initializer typecheck 现与局部 `val` pattern binding 对齐：有整体类型注解时按 expected-type 校验，无整体注解时直接以 initializer 推断出的 subject 类型驱动 `val_pat::infer_val_pat_bindings`，并把 binder 类型写回 side table 供后续 lowering 复用。
  - 已新增回归：
    - `tests/fixtures/typecheck/top_level_val_pattern_inferred_same_file_ok.scoop`
    - `tests/fixtures/typecheck_multi/top_level_val_pattern_inferred_cross_file/defs_tuple.scoop`
    - `tests/fixtures/typecheck_multi/top_level_val_pattern_inferred_cross_file/defs_struct.scoop`
    - `tests/fixtures/typecheck_multi/top_level_val_pattern_inferred_cross_file/defs_enum.scoop`
    - `tests/fixtures/typecheck_multi/top_level_val_pattern_inferred_cross_file/use.scoop`
- 已验证：
  - `cargo test -p scoopc top_level_`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (329)`）
  - `cargo run -p scoop -- test --fixtures /tmp/t4004a2-typecheck-multi`（临时 fixtures root，仅包含 `top_level_val_pattern_inferred_cross_file` case，`fixtures: ok (4)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4004a1

### T4004b [DONE] 打通顶层 `val` pattern binder 的 HIR / LLVM once-init lowering
- 范围：
  - 顶层 pattern initializer 只求值一次；各 binder 复用统一投影结果，不得把 initializer 重复展开到每个 binder。
  - 非 `const` 顶层 binder 复用 `top_level_immutable_values` 主线，保持初始化顺序、可见性与循环引用失败路径稳定。
  - 新增 lowering / run-pass 回归，覆盖 tuple / struct / enum 顶层 binder 读取。
- 验收：
  - 顶层 binder 在 `main`、其它顶层 initializer 与跨文件调用中可稳定 build/run。
- 完成：
  - HIR lowering 现会把顶层 pattern `val` 展开成“隐藏 subject + 可选隐藏 check + 可见 binder”的一组顶层 immutable value：subject once-init 负责 initializer 求值，variant 路径额外通过隐藏 `Unit` check 值统一复用运行期匹配失败语义，binder 自身继续复用局部 destructuring 已有的投影 / `when` 提取 helper，不再保留匿名且不可执行的顶层值形态。
  - 顶层 pattern binder 现统一进入 `top_level_immutable_values` side table，并直接复用普通顶层 `val` 的 once-init / guard / 递归初始化失败主线；同文件 `main`、其它顶层 initializer 与 cone 多文件跨文件读取都可稳定 build/run。
  - 已新增回归：
    - Rust 单测 `lower_typed_single_source_file_expands_top_level_pattern_into_hidden_subject_and_check`
    - `tests/fixtures/run-pass/top_level_val_pattern_runtime_basic.scoop`
    - `tests/fixtures/run_pass_cone/top_level_val_pattern_multi_file_basic/**`
- 已验证：
  - `cargo test -p scoopc top_level_`
  - `cargo run -p scoop -- test --fixtures <临时 fixtures root（仅包含 run-pass/top_level_val_pattern_runtime_basic）>`（`fixtures: ok (1)`）
  - `cargo run -p scoop -- test --fixtures <临时 fixtures root（仅包含 run_pass_cone/top_level_val_pattern_multi_file_basic）>`（`fixtures: ok (1)`）
  - `cargo run -p scoop -- build <临时 probe>/main.scoop -o <临时 probe>/a.out`，随后执行产物返回 `3`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4004a2

### T4004R [DONE] Review：确认顶层与局部 pattern binding 复用同一套语义
- 重点：
  - 不接受“顶层单独走一套 ad-hoc lowering”。
- 完成：
  - 已复审 `lower_top_level_pattern_val_items -> synth_pattern_runtime_check_expr / synth_pattern_binding_init_expr` 与 `lower_local_pattern_val_stmt` 主线：顶层 pattern binder 的运行期校验、tuple/struct/variant 投影与 payload 提取直接复用局部 destructuring helper，没有新增顶层专用的 pattern 投影或匿名值读取旁路。
  - 复审中发现并修复一个既有的 state-machine 裂缝：`expr_contains_suspend_subtree` 先前把所有 `VarRef` 一概视为“不会隐藏 suspend”，导致 `boomY + 1` 这类“顶层 pattern binder 嵌在更大表达式中”的场景不会生成 `TopLevelValueInitAccess` site；同时，state-machine emitter 也漏把 `TopLevelValueInitAccess` 纳入与 `ObjectInitAccess` 对齐的 inactive/active 分支。现已让 hidden suspend var ref 进入统一 plan，并让 state-machine 环境下的 `codegen_top_level_immutable_value_access` 在 init call 后若 effect 已 active，则跳过 guard/load，交由外层 boundary 统一 dispatch。
  - 已新增 `tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop` 回归，覆盖：
    - 顶层 pattern binder 在 handle 中匹配成功时，once-init boundary 走 inactive path，后续 `+ 1` 与 caller tail 继续执行；
    - 顶层 pattern binder 在 handle 中 mismatch 时，隐藏 check 触发的 `Raise.raise(RuntimeError.NullAssertionFailed)` 会命中 handler，而不会继续执行 tail 或 `exit(1)`。
- 已验证：
  - `target/debug/scoop run tests/fixtures/run-pass/effect_handle_top_level_val_pattern_access_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/top_level_val_pattern_runtime_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/object_init_raise_try_catch_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/effect_handle_object_init_access_inactive_basic.scoop`
  - `target/debug/scoop run tests/fixtures/run-pass/class_init_hidden_raise_helper_try_catch_basic.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4004b

## T4005：Elvis `?:` lowering / codegen

### T4005 [DONE] 把 Elvis `?:` 从静态规则推进到可执行 lowering / codegen
- 范围：
  - HIR lowering 不再落回 `Any` fallback。
  - LLVM codegen 支持 Elvis 主路径。
  - nullable / rhs type 规则与执行语义保持一致。
- 已完成：
  - HIR lowering 现将 Elvis 统一 desugar 为 `when (lhs) { Some(v) -> v; None -> rhs }`，保证 lhs 只求值一次、rhs 仅在 `None` 分支求值，不再把 `?:` 留在通用 `Binary` 的 `Any` fallback 上。
  - typed lowering 现为 `?.` / safe-call / `!!` / Elvis 这条 nullable desugar 主线统一写回精确结果类型；LLVM `When` codegen 也会使用表达式静态类型作为结果 expected-context，修复 `Any` 结果和 tuple element 上下文里 Elvis 落入 `when arm type mismatch` 的既有裂缝。
  - 已新增回归：
    - `tests/fixtures/hir/elvis_lowering.scoop`
    - `tests/fixtures/run-pass/elvis_lazy_basic.scoop`
    - `tests/fixtures/run-pass/elvis_any_tuple_context_basic.scoop`
- 验收：
  - 对应 fixtures 从 typecheck 扩展到 run-pass。
  - `ISSUES.md` 第 13 条收窄或关闭。
- 已验证：
  - `cargo run -p scoop -- test --fixtures tests/fixtures/hir`（`fixtures: ok (17)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (329)`）
  - 定向 Elvis fixtures root（`fixtures: ok (4)`，覆盖 HIR / run-pass / typecheck）
  - safe member access 回归 root（`fixtures: ok (1)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4004R

### T4005R [DONE] Review：确认 Elvis 不再停留在“语法通过但不可执行”
- 重点：
  - 不允许保留 parser/typecheck 接受、lowering/codegen 拒绝的裂缝。
- 完成：
  - 复审 parser / typecheck / HIR / LLVM 后确认，Elvis 主线已统一收口为 nullable desugar：前端不再把 `?:` 保留到可执行后端的 `Binary(Elvis)` 形态，LLVM 侧也不再依赖单独的 Elvis 二元运算分支才能执行。
  - 复审同时发现并修复两条残留裂缝：
    - typecheck 先前仍把 rhs 当成“无 expected type 的独立表达式”推断，导致 `val xs = noneArray ?: []` 错误报 `array_lit_type_annotation_required`；
    - Elvis 降成 `when` 后，`when` arm 的 expected-context codegen 仍漏接 `Closure`，导致 `val f = noneThunk ?: { 7 }` 在 LLVM 阶段落入 `expression kind` unsupported。
  - 现已把 Elvis rhs 统一改为使用 lhs nullable inner type 做 expected-context typecheck，并让 `codegen_expr_in_expected_context` 直接接到 `codegen_closure_expr`，使空数组字面量与 lambda rhs 都能沿统一主线可执行。
  - 已新增 `tests/fixtures/run-pass/elvis_rhs_expected_context_basic.scoop` 回归，覆盖“不依赖外层变量注解、仅靠 lhs inner type 驱动 `[]` 与 `{ 7 }` 定型”的两条路径。
- 已验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/elvis_rhs_expected_context_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/elvis_lazy_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/elvis_any_tuple_context_basic.scoop`
  - 临时 probe：`safe-call + Elvis` 组合执行仍保持 lhs 单次求值与 rhs 惰性
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (329)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4005

### T4005S [DONE] 收口 `when` / pattern binder 中函数值的可调用 lowering / codegen
- 说明：
  - 在执行 `T4005R` 的扩展 probe 时，发现一个独立于 Elvis 本身的既有裂缝：`when (some) { Some(g) -> g(); None -> ... }` 这类“pattern binder 承载函数值并立即调用”的场景，当前会在 LLVM 阶段报 `call callee` unsupported。
  - 该问题说明 callable-value 主线虽然已覆盖普通局部、顶层函数值与 funptr，但对 pattern binder 引入的函数值仍有 lowering / codegen 元数据缺口；若不显式入表，后续继续推进 compilation-unit / RTTI / effect 任务时会把这个核心调用语义裂缝继续带下去。
- 范围：
  - `when` / 其它 pattern binder 引入的函数值，需要在 HIR / LLVM 主线上保持可调用元数据，不再退化成不可调用的普通 local ref。
  - 新增最小 run-pass 回归，覆盖 `Some(f) -> f()` 一类 pattern binder callable 场景。
- 已完成：
  - typecheck 现会把 `when_pat::infer_when_pat_bindings` 的结果写回 `inferred_binding_tys` side table，不再只在 arm 局部环境里临时可见。
  - HIR lowering 新增 `when_pat_binding_tys` side table，并在 source / synthetic `when` binder 位置记录精确 `TypeId`，供后端恢复 callable-value 元数据。
  - LLVM `bind_when_pat` 现按“当前源文件 + binder span”回查 binder 的 `hir_ty`，并同步恢复基于类型的 `call_may_suspend`；`Some(f) -> f()` 与 `(g, n) -> g() + n` 不再退化成 `call callee` unsupported。
  - 已新增 `tests/fixtures/run-pass/when_pattern_function_value_call_basic.scoop`，同时覆盖 variant binder 与 tuple binder 中函数值立即调用。
- 验收：
  - pattern binder 引入的函数值与普通局部函数值保持一致，可稳定 build/run。
- 已验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/when_pattern_function_value_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures <临时 root，仅包含 when_pattern_function_value_call_basic>`（`fixtures: ok (1)`）
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (329)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4005R

### T4005T [DONE] 收口顶层 callable value（含顶层 pattern binder / `FunPtr`）的调用语义
- 说明：
  - 在执行 `T4005SR` 的扩展 probe 时，确认局部 destructuring 与 `when` binder 的函数值调用已经走通，但顶层 callable value 主线仍有独立裂缝：
    - `val topF: () -> Int = { 11 }; fun main() { topF() }` 当前在 typecheck 阶段报 `callee_not_callable`；
    - `val (topF, topN): (() -> Int, Int) = ({ 11 }, 4); topF()` 同样报 `callee_not_callable`；
    - 顶层 `FunPtr` direct call 可通过 typecheck，但 LLVM codegen 仍报 `call callee type`。
  - 这说明 `T4005S` 只补齐了局部 / `when` pattern binder 的 callable-value 元数据；顶层 immutable value 仍没有接入统一 callable-value call 路径，导致顶层 pattern binder 只是碰巧暴露了一个更基础的顶层调用缺口。
- 范围：
  - 顶层命名 `val` 与顶层 pattern binder 产出的函数值，需要像局部函数值一样可直接调用。
  - 顶层 `FunPtr` direct call 也需要进入同一套 callable-value lowering / codegen 主线，而不是继续只按“顶层函数名”处理。
  - 新增最小 run-pass 回归，覆盖顶层命名函数值、顶层 pattern binder 函数值与顶层 `FunPtr` direct call。
- 验收：
  - 上述三类 probe 均可稳定 build/run，不再出现 `callee_not_callable` 或 `call callee type`。
- 已完成：
  - typecheck `infer_call_expr_type` 现会在未命中 `top_level_funs` 时回查 `top_level_types`：顶层命名 `val` 与顶层 pattern binder 产出的函数值会直接复用现有的函数值调用检查，顶层 `FunPtr` 仍复用同一套 direct-call 校验，不再只对 `FunPtr` 留单独旁路。
  - LLVM `codegen_call` 现会先识别顶层 callable value 的精确 `TypeId`，再把顶层值读取接回既有的函数值 / `FunPtr` 间接调用 helper；`ValueRef::TopLevel` 的读取路径也已收口到统一 `codegen_top_level_value_ref`，不再把 callable top-level value 一律误当成“普通顶层函数名”。
  - 在补顶层 pattern binder callable 路径时，还顺带修复了一个同源的既有 lowering/codegen 裂缝：tuple literal 元素先前没有按元素类型进入 expected-context，导致 `({ 7 }, 4)` 这类含 closure literal 的 tuple 会在 LLVM 阶段落入 `expression kind` unsupported；当前 tuple literal 已改为和 struct literal 一样按元素类型驱动元素 codegen。
  - 已新增 `tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`，同时覆盖：
    - 顶层命名函数值 direct call；
    - 顶层 pattern binder 函数值 direct call；
    - 顶层 `FunPtr` direct call；
    - 上述 callable value 在其它顶层 initializer 中的调用。
- 已验证：
  - `cargo fmt --all`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures /tmp/t4005t-fixtures`（`fixtures: ok (1)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4005S

### T4005SR [DONE] Review：确认 callable-value 主线已覆盖 pattern binder
- 重点：
  - 不允许只让普通局部 `val f = ...; f()` 与 `when` binder `Some(f) -> f()` 可调用，而顶层 pattern binder / 顶层 callable value 仍走另一套 typecheck 或 codegen 失败路径。
- 完成：
  - 复审 `T4005S/T4005T` 后确认，局部 destructuring、`when` binder、顶层命名 `val`、顶层 pattern binder 与顶层 `FunPtr` 现已统一走 callable-value 主线；`infer_call_expr_type` 不再把顶层 callable value 误判为不可调用，LLVM `codegen_call` 也不再把 `ValueRef::TopLevel` 一概当成普通顶层函数名。
  - 复审中发现并修复一个剩余的 typecheck 裂缝：`infer_expr_type_in_expected_context` 先前没有为 tuple literal 向下传播 expected element type，导致顶层 pattern binder initializer `val (f, n): (String.(Int) -> Int, Int) = ({ ...this... }, 3)` 中的 receiver lambda 仍按“无 expected type”检查，报 `unknown_local_value_type: this`。现已新增 tuple expected-context 分支，让 tuple 元素逐个复用 `infer_in_expected(...)`，receiver function type 会正确下传给 lambda 元素。
  - 已新增 `tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop` 回归，同时覆盖：
    - 顶层命名 receiver function value；
    - 顶层 pattern binder 中的 receiver function value；
    - 局部 destructuring binder；
    - `when` pattern binder；
    - 顶层 `FunPtr` direct call；
    - 上述调用统一使用 receiver + named args，确认主线没有重新按 callee 形态分叉。
- 已验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/callable_value_pattern_binder_receiver_named_args_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/top_level_callable_value_call_basic.scoop`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/when_pattern_function_value_call_basic.scoop`
  - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (329)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4005T

## T4006：跨文件 / 跨包编译链路

### T4006 [DONE] 收口跨文件顶层值、跨文件实例化与跨包扩展解析
- 范围：
  - 顶层值类型表不再只看当前文件。
  - 单态化 lowering 支持跨文件顶层函数实例化。
  - 扩展函数解析不再限于同包。
- 验收：
  - 新增多文件 / 多包 regression。
  - `ISSUES.md` 第 14 条收窄或关闭。
- 已完成：
  - 新增 `tests/fixtures/run_pass_cone/cross_file_generic_top_level_val_basic`，覆盖同一 cone 中“跨文件顶层 `val` 读取 + 非入口文件顶层泛型函数实例化”主线，确认 `helperBase/helperDelta/helperSummary` 这类非入口文件绑定与 `id<T>` 的跨文件实例化都能稳定 build/run。
  - 新增 `tests/fixtures/typecheck_cone/cross_cone_extension_imports`，覆盖跨 cone / 跨包 extension 的显式 import 与 star import 两条 typecheck 主线；同时复验既有 `tests/fixtures/resolve_cone/extension_imports`，确认 resolver 候选收集与导入语义仍一致。
  - 同步更新 `ISSUES.md` 与相关注释，收口“顶层值只看当前文件 / monomorph 只看当前文件 / extension 只看同包”的过时说法，并明确单文件 `dump-ir` helper 不再计入 compilation-unit issue。
- 已验证：
  - `cargo run -p scoop -- test --fixtures /tmp/t4006-runpass.negqIs`（`fixtures: ok (1)`）
  - `cargo run -p scoop -- test --fixtures /tmp/t4006-typecone.3DoKIZ`（`fixtures: ok (3)`）
  - `cargo run -p scoop -- test --fixtures /tmp/t4006-resolve2.iuHi5G`（`fixtures: ok (6)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4005SR

### T4006S [DONE] 修复 delegated property lazy(None) 读取进入 `print/println` 时的 codegen 类型缺口
- 说明：
  - 在完成 `T4006` 的全量验证时，`cargo run -p scoop -- test` 当前会失败在 `tests/fixtures/run-pass/delegated_property_lazy_thread_safety_none_single_thread_ok.scoop`，错误为 `scoop::llvm::unsupported_main_body: sysroot print/println arg type`。
  - 该失败与 `T4006` 的 compilation-unit 改动无直接交叉，但它说明 delegated property lazy getter 的 HIR / codegen 类型传递仍有既有裂缝；若不先收口，后续 review 与阶段验收会继续停在红基线上。
- 范围：
  - 修复 `val x: Int by lazy(LazyThreadSafetyMode.None) { ... }` 一类读取在 `println(x)` 等调用位置的结果类型丢失问题。
  - 为 delegated property lazy(None) 的读取 + 打印路径补充定向 regression。
  - 重新验证 `cargo run -p scoop -- test` 不再被该用例阻断。
- 完成：
  - `lower_lazy_delegated_property_get_from_receiver` 现已返回真实属性 `TypeId`；`LazyThreadSafetyMode.None` 生成的 getter `when` 不再把外层表达式类型写成 `Any`，从而避免 `print/println` 路径把 `Int`/`Bool` 等值错误装箱成通用引用。
  - 已新增 run-pass 回归 `tests/fixtures/run-pass/delegated_property_lazy_thread_safety_none_print_like_ok.scoop`，覆盖 `lazy(None)` 读取先走 `print`、再走 `println` 的 print-like lowering 共用路径。
  - 重新验证后，`cargo run -p scoop -- test` 已不再卡在 `delegated_property_lazy_thread_safety_none_single_thread_ok.scoop`；完整套件继续向后推进时又暴露出新的既有 blocker `gc_continuation_cross_thread_resume_with_objects.scoop`，已前插为 `T4006T`。
- 已验证：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/delegated_property_lazy_thread_safety_none_single_thread_ok.scoop -o /tmp/t4006s_lazy_none.out`
  - `/tmp/t4006s_lazy_none.out`（stdout：`init / 7 / 7`）
  - `cargo run -p scoop -- build tests/fixtures/run-pass/delegated_property_lazy_thread_safety_none_print_like_ok.scoop -o /tmp/t4006s_lazy_print_like.out`
  - `/tmp/t4006s_lazy_print_like.out`（stdout：`init / 7,7`）
  - `cargo run -p scoop -- build tests/fixtures/run-pass/delegated_property_lazy_init_once_basic.scoop -o /tmp/t4006s_lazy_default.out`
  - `/tmp/t4006s_lazy_default.out`（stdout：`before / init / 7 / 7`）
  - `cargo run -p scoop -- build tests/fixtures/run-pass/delegated_property_lazy_thread_safety_synchronized_once.scoop -o /tmp/t4006s_lazy_sync.out`
  - `/tmp/t4006s_lazy_sync.out`（stdout：`7 / 7 / init_calls=1 / ok`）
  - `cargo run -p scoop -- build tests/fixtures/run-pass/delegated_property_lazy_thread_safety_publication_multi_init.scoop -o /tmp/t4006s_lazy_pub.out`
  - `/tmp/t4006s_lazy_pub.out`（stdout：`7 / 7 / init_started=2 / ok`）
  - `cargo run -p scoop -- test`（本用例已清除；套件后续在 `run-pass/gc_continuation_cross_thread_resume_with_objects.scoop` 触发新的既有 `scoop::llvm::unsupported_main_body: value coercion`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4006

### T4006T [DONE] 修复 `gc_continuation_cross_thread_resume_with_objects` 的 codegen `value coercion` 缺口
- 说明：
  - 在 `T4006S` 收口后重新跑 `cargo run -p scoop -- test`，套件继续向后推进到 `tests/fixtures/run-pass/gc_continuation_cross_thread_resume_with_objects.scoop` 时失败。
  - 该夹具当前不是运行期 crash，而是在 `cargo run -p scoop -- build tests/fixtures/run-pass/gc_continuation_cross_thread_resume_with_objects.scoop -o /tmp/t4006s_gc_cont.out` 阶段报 `scoop::llvm::unsupported_main_body: value coercion`。
  - 该 failure 属于 effect / continuation + GC 对象图跨线程恢复路径的既有 codegen 缺口；若不先修复，`T4006R` 的全量 fixture review 仍无法建立在绿基线上。
- 范围：
  - 精确定位该夹具里是哪一段 `handle/when/Option/object graph` 流程仍落入 LLVM `value coercion` unsupported。
  - 修复对应 lowering / codegen 类型传递缺口，保证该夹具至少能稳定 build/run 到既定 golden。
  - 重新验证 `cargo run -p scoop -- test` 不再被该夹具阻断。
- 已完成：
  - 根因定位到 `codegen_class_ctor_eval_args`：`ClassInit` side table 由独立 lowering pass 生成，ctor 参数 `SymbolId` 与主 HIR 调用点 locals 不共享同一编号空间；旧实现会在“显式实参尚未全部求值完”时把 ctor 参数提前塞进 `env`，导致 `return Node(name, t, value)` 这类调用在求值后续实参时把调用者局部变量误读成 ctor 参数，最终落入 `String -> Int` 的 `value coercion` unsupported。
  - 现已把 class ctor / super ctor / `this(...)` delegation 的实参求值改成两阶段：先在调用者环境中按源码顺序求值全部显式实参，再进入 ctor 参数作用域绑定这些显式值，并在该作用域里补齐默认值。这样默认值仍能读取已提供参数，但显式实参求值不再受 side-table `SymbolId` 污染。
  - 已新增 focused run-pass 回归 `tests/fixtures/run-pass/class_ctor_arg_eval_scope_shadow_free_basic.scoop`，直接覆盖“helper 函数局部 + struct 临时值 + class ctor call”这条最小主线。
  - `gc_continuation_escape_deep_object_graph.scoop` 与 `gc_continuation_cross_thread_resume_with_objects.scoop` 现已都能稳定 build/run；`cargo run -p scoop -- test` 也已越过原先的 `gc_continuation_cross_thread_resume_with_objects` 红线，并继续向后暴露出新的既有 blocker `top_level_val_recursive_init_is_error.scoop`，已前插为 `T4006U`。
- 已验证：
  - `cargo run -p scoop -- build tests/fixtures/run-pass/gc_continuation_escape_deep_object_graph.scoop -o /tmp/t4006t_gc_deep.out`
  - `/tmp/t4006t_gc_deep.out`（stdout 与 `gc_continuation_escape_deep_object_graph.stdout` 一致）
  - `cargo run -p scoop -- build tests/fixtures/run-pass/gc_continuation_cross_thread_resume_with_objects.scoop -o /tmp/t4006t_gc_cross.out`
  - `/tmp/t4006t_gc_cross.out`（stdout 与 `gc_continuation_cross_thread_resume_with_objects.stdout` 一致）
  - `cargo run -p scoop -- run tests/fixtures/run-pass/class_ctor_arg_eval_scope_shadow_free_basic.scoop`
  - `cargo run -p scoop -- test --fixtures <临时 fixtures root（仅包含 class_ctor_arg_eval_scope_shadow_free_basic / gc_continuation_escape_deep_object_graph / gc_continuation_cross_thread_resume_with_objects）>`（`fixtures: ok (3)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4006S

### T4006U [DONE] 修复 full fixture suite 中 `top_level_val_recursive_init_is_error` 的陈旧 stdout golden mismatch
- 说明：
  - `T4006T` 收口后重新跑 `cargo run -p scoop -- test`，套件稳定失败在 `tests/fixtures/run-pass/top_level_val_recursive_init_is_error.scoop` 的 stdout golden 比对。
  - 复查后确认失败根因并非顺序污染：`target/debug/scoop run .../top_level_val_recursive_init_is_error.scoop` 的实际行为一直是“退出码 `1`，stdout/stderr 为空”，而 `top_level_val_recursive_init_is_error.stdout` 自 `T4003SR` 起保留了一个单独的换行符，变成了陈旧 golden。
- 范围：
  - 精确定位该 fixture 的实际 stdout 与 golden 期望差异来源。
  - 修正 fixture 期望，使其与“递归初始化在进入 `main` 前即终止，因此 stdout 为空”的当前语义一致。
  - 重新验证 `cargo run -p scoop -- test` 不再被该 fixture 阻断。
- 完成：
  - 已确认 `top_level_val_recursive_init_is_error` 的真实输出为“无 stdout + 退出码 `1`”；问题根因是 golden 文件仍保留历史换行，而不是 harness / 顺序污染。
  - 已将 `tests/fixtures/run-pass/top_level_val_recursive_init_is_error.stdout` 收口为空文件，并在 fixture 注释中明确“程序在进入 `main` 前终止，因此 stdout 为空”的语义。
  - 重新跑完整 fixture suite 后，`cargo run -p scoop -- test` 现已全绿，不再被该 fixture 阻断。
- 已验证：
  - `target/debug/scoop test --fixtures <仅含 top_level_val_recursive_init_is_error 的临时 root>`（`fixtures: ok (1)`）
  - `target/debug/scoop test --fixtures tests/fixtures/run-pass`（`fixtures: ok (346)`）
  - `cargo run -p scoop -- test`（`fixtures: ok (1051)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4006T

### T4006V [DONE] 收口链式成员访问在非局部 receiver 上的解析 / codegen
- 说明：
  - 在为 `T4006T` 添加 focused regression 时，最小 probe `println(node.tag.label)` 暴露出一个独立既有缺口：HIR 中外层 `label` 访问仍会保留 `member.resolved = None`，LLVM `codegen_member_access` 随后报 `scoop::llvm::unsupported_main_body: member access target`。
  - 该问题与 ctor 实参求值污染不是同一根因；当前已把 focused regression 改成复用 `describeNode(node)` 路径，避免把两个问题混在一个任务里。
- 范围：
  - 让 `obj.field.subfield` / `obj.structField.member` 这类链式成员访问在 typecheck / HIR / LLVM 主线上稳定解析并执行。
  - 补齐对应的最小 regression，覆盖“receiver 不是局部 struct slot，而是另一个 member access 结果值”的路径。
- 完成：
  - 已定位根因：resolver 只会为裸 ident / 少数特殊 receiver 写回成员绑定，`holder.node.tag.label` 这类普通值成员链在外层 `label` 上不会留下 `member.resolved`；而普通 `MemberAccess` / assignment lhs 的 typecheck 先前也不像 `?.` 那样按已推导 receiver 类型做 late resolve，导致显式类型上下文会前置报 `member access（未 resolve）`，直接 `println(...)` 则把 `resolved = None` 漏进 HIR，最终在 LLVM 报 `member access target`。
  - 已在 `typecheck::expr::member` 收口共享 helper `resolve_member_value_target_for_receiver`，统一普通 member access、safe member access 与 assignment lhs 的“resolver 结果 + 基于 receiver 类型的晚解析”主线；receiver lambda 的隐式 `this` 仍优先使用晚解析结果，避免回退到旧 `this` 绑定。
  - 已新增 HIR 单测 `lower_typed_single_source_file_preserves_chained_member_access_resolution`，确认 `holder.node.tag.label` / `holder.node.tag.score` 不再把外层成员保留为 unresolved；并新增 run-pass 回归 `tests/fixtures/run-pass/chained_member_access_non_local_receiver_basic.scoop`，覆盖显式类型上下文与 `println(holder.node.tag.label)` 直通 codegen 的执行路径。
- 已验证：
  - 最小 probe `println(node.tag.label)` 现可 `cargo run -p scoop -- build <tmp>.scoop -o /tmp/t4006v_probe.out` 并执行输出 `alpha`
  - `cargo test -p scoopc preserves_chained_member_access_resolution`
  - `cargo run -p scoop -- test --fixtures <临时 fixtures root（仅含 chained_member_access_non_local_receiver_basic）>`（`fixtures: ok (1)`）
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (1052)`）
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4006U

### T4006R [DONE] Review：确认 compilation-unit 维度规则已统一
- 重点：
  - 不允许只靠“入口文件特权”维持通过。
- 完成：
  - 复审 `build -> typecheck -> lower_for_compilation_unit_multi_files -> reachability/codegen -> effect state-machine` 主线后，确认此前仍残留一个真实的入口文件特权裂缝：`ctor_call_sites` 与 `continuation_resume_call_sites` 只按裸 `Span` 建索引，multi-file lowering 因而只保留入口文件 side table；非入口文件中的 ctor 调用会在 codegen 侧漏失绑定，误落入 enum variant ctor fallback，而 `Continuation.resume` 也只能在入口文件里被 effect segmentation 识别。
  - 已将两类调用点 side table 统一收口为 source-aware 的 `hir::CallSite { source_path, span }` 键，并让 HIR lowering、reachability、LLVM codegen、known-fun suspend 分析与 unified state-machine plan 全部按“当前源码文件 + span”查询，不再依赖入口文件特权。
  - 已新增 HIR 单测 `lower_for_compilation_unit_multi_files_preserves_non_entry_call_site_side_tables`，直接断言非入口文件中的 ctor 调用点与 `Continuation.resume` 调用点都会进入全局 side table；并新增 cone run-pass 回归 `tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic/**`，同时覆盖 helper 函数、object init、class init 三条非入口文件 ctor 路径。
- 已验证：
  - `cargo test -p scoopc lower_typed_single_source_file_records_statement_position_continuation_resume_call_site -- --nocapture`
  - `cargo test -p scoopc lower_for_compilation_unit_multi_files_preserves_non_entry_call_site_side_tables -- --nocapture`
  - `cargo run -p scoop -- build tests/fixtures/run_pass_cone/cross_file_ctor_named_default_basic -o /tmp/t4006r_cross_file_ctor.out`
  - `/tmp/t4006r_cross_file_ctor.out`（stdout 为 `42 / 10:7 / 10:9`）
  - `cargo run -p scoop -- test`（`fixtures: ok (1053)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4006V

## T4007：RTTI 参数化支持

### T4007 [DONE] 将 RTTI 参数化支持拆分为“generic class target / parameterized interface+`eff` / 旧 RTTI 导出 API”三段执行
- 说明：
  - 原任务同时覆盖三条独立主线：generic class 的 `is/as/as?` 运行期 descriptor 选择、parameterized interface 与 `eff` 参数 target 的运行期匹配、以及 `crates/scoopc/src/rtti/mod.rs` 旧布局导出 API 的 `unsupported_generic_type` 门禁。
  - 临时 probe 复现表明这三条主线并非同一根因：一是 `Any` 上做 `is Holder<Int>` 会在 LLVM 侧退回 base generic class descriptor，触发 `TypeKind::Param(T)` / `class field type`；二是 `Disposable<eff Raise<RuntimeError>>` 的实例在运行期做 `is Disposable<eff Pure>` 仍错误返回 `true`，说明 itable 仍只按 base interface id 判真；三是旧 RTTI 导出 API 仍直接拒绝 args / `eff` 参数化 nominal。
  - 为保证每轮只提交一个闭环切片，现拆分为 `T4007a -> T4007b -> T4007c -> T4007S -> T4007R` 顺序推进。
- 完成：
  - 已完成任务拆分，generic class target、parameterized interface / `eff` runtime match、旧 RTTI 导出 API，以及随后插入的全量 Rust 测试验证 blocker `T4007S` 均已按顺序收口。
- 依赖：T4006R

### T4007a [DONE] 为 generic class 的 `is/as/as?` 使用具体实例化 descriptor
- 范围：
  - 参数化 class target 的运行期 type test / cast 不再退回 base generic class descriptor。
  - `Holder<Int>` / `Holder<UInt>` 这类 instantiated class target 可稳定 build/run，并在 `is/as/as?` 正反路径上得到正确结果。
  - parameterized interface / `eff` target 与旧 RTTI 导出 API 继续留给后续子任务。
- 验收：
  - 新增 run-pass 回归，覆盖 generic class `is/as/as?` 正反路径。
  - `dump-rtti --type 'Holder<Int>'` 能导出具体实例化 descriptor。
- 完成：
  - LLVM `codegen_ref_is_instance_of_nonnull` 现对带 type args 的 class target 优先按 `nominal_layout_key` 查找实例化后的 `ClassInit` / type descriptor，不再先命中 base generic class。
  - generic class 的 `is` / `as` / `as?` 现在统一复用具体实例化 descriptor 路径；`Holder<Int>` 的正向检查和 `Holder<UInt>` 的反向检查都已可执行。
  - 已新增 run-pass 回归：
    - `tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop`
    - `tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.stdout`
- 已验证：
  - `cargo run -p scoop -- run tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop`
  - `cargo run -p scoop -- dump-rtti tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop --type 'Holder<Int>'`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4006R

### T4007b [DONE] 为 parameterized interface 与 `eff` 参数 target 补齐运行期匹配
- 范围：
  - 带 type args / `eff` row 的 interface target 不再只按 base interface id 判真。
  - `Disposable<eff Raise<RuntimeError>>`、`Disposable<eff Pure>` 一类运行期判断结果与前端 assignable / effect-row 包含关系一致。
  - 若 generic / `eff` supertype 链会影响 runtime match，需要同步补齐对应 metadata。
- 验收：
  - 新增 run-pass / RTTI 定向回归，覆盖 `eff` 参数 target 的正反路径。
  - `ISSUES.md` 第 15 条至少收窄到“仅剩旧 RTTI 导出 API”或被完全关闭。
- 完成：
  - `crates/scoopc/src/itable.rs` 现为每个 concrete class 的 interface itable entry 同步保存 `interface_type_name` / `interface_type_id` 与 `runtime_match_type_names` / `runtime_match_type_ids`，运行期匹配集合复用 `TypeLowering::instantiated_direct_supertypes(...)` 与 `is_type_assignable(...)` 预计算。
  - LLVM `is/as/as?` 针对 interface target 已改为扫描 `runtime_match_type_ids`，不再只按 base `interface_id` 判真；interface method dispatch 仍继续按 base `interface_id + slot` 工作。
  - `dump-rtti` 现能导出 parameterized interface / `eff` target 的精确 `interface_type_name` 与 `runtime_match_type_names`，并补上对应单元回归。
  - 已新增 run-pass 回归：
    - `tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop`
    - `tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.stdout`
- 已验证：
  - `cargo check --all-targets`
  - `cargo run -p scoop -- run tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop`
  - `cargo run -p scoop -- dump-rtti tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop --type 'StringReadable'`
  - `cargo run -p scoop -- dump-rtti tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop --type 'PureManaged'`
  - `cargo test -p scoopc rtti::type_desc::tests`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4007a

### T4007c [DONE] 收口旧 RTTI 导出 API 的参数化类型支持与文档同步
- 范围：
  - `crates/scoopc/src/rtti/mod.rs` 不再对 generic / `eff` 参数化 nominal 直接报 `unsupported_generic_type`。
  - 旧 RTTI 导出 API 的 canonical name / type_id 口径与 `dump-rtti` 当前 type descriptor 输出保持一致。
  - 如有必要，同步 `SCOOP_RUNTIME.md` / `SCOOP_FULL_SPEC.md` 的 RTTI 可观测性描述。
- 验收：
  - 新增 RTTI 定向测试与必要的文档同步。
- 完成：
  - 旧 RTTI `dump_type_rtti` 现改为通过 synthetic type query + `TypeLowering::new_with_ctx(...)` 在“当前文件 package/import 语境”下解析查询，因此 `Readable<String>`、`Disposable<eff Raise<RuntimeError>>`、`Pair<Int>` 一类带 type args / `eff` row 的 nominal 都能直接导出，不再早退到 `unsupported_generic_type`。
  - `rtti/mod.rs` 现会收集当前编译单元（含 sysroot）的 struct 声明头，并在导出参数化 struct RTTI 时通过 `lower_type_ref_in_decl_file_with_scopes(...)` 重新按声明处文件上下文实例化字段类型；因此 generic struct 的字段类型、offset、size/align 现在都和 concrete use-site 一致。
  - `nominal_layout(...)` 已去掉对 args / `eff` 的早期硬拒绝：class/interface/effect 继续走 pointer layout，enum 继续走 word-sized fallback，struct 则使用实例化后的字段类型计算布局；旧 RTTI 的 canonical name / type_id 仍统一由 `TypeStore::display` + `stable_hash64` 生成。
  - 已新增 2 条旧 RTTI 单测，分别覆盖参数化 struct 字段实例化，以及 parameterized interface / `eff` target 与 `type_desc` metadata 的 canonical name / type_id 对齐；同时补充 `SCOOP_RUNTIME.md`，明确参数化 nominal 的 canonical name 需包含 concrete type args 与 use-site `eff` row。
- 已验证：
  - `cargo test -p scoopc rtti:: -- --nocapture`
  - `cargo run -p scoop -- dump-rtti tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop --type 'StringReadable'`
  - `cargo run -p scoop -- dump-rtti tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop --type 'Holder<Int>'`
  - `cargo clippy --all-targets -- -D warnings`
  - 已尝试 `cargo test --all`，但被既有 runtime 测试 `crates/scoop_runtime/tests/gc_immix_compaction.rs` 中两条 compaction 用例挂起阻断；该问题已显式入列为 `T4007S`，不会静默略过。
- 依赖：T4007b

### T4007S [DONE] 排查并修复 `cargo test --all` 中 `gc_immix_compaction` 的既有挂起
- 说明：
  - 在 `T4007c` 验证阶段，`cargo test --all` 会稳定卡在 `crates/scoop_runtime/tests/gc_immix_compaction.rs` 的
    `immix_compaction_does_not_move_pinned_objects` 与
    `immix_compaction_updates_native_roots_slots_and_object_fields`；
    输出会反复停留在 `waiting for park: epoch=2 parked=0 need=1`，超过 60 秒不前进。
  - 该挂起与本轮 RTTI 导出代码路径无直接交集，但它阻断了当前基线对全量 Rust 测试的完成验证，不能继续被当成“与当前任务无关”而忽略。
- 范围：
  - 定位并修复上述两条 compaction 测试的 STW / park 协调挂起。
  - 恢复 `cargo test --all` 可完整跑通。
- 完成：
  - 复查 `crates/scoop_runtime/tests/gc_immix_compaction.rs` 与 `runtime/c/scoop_gc_backend_immix.c` 的线程注册、`enter_native/leave_native`、STW begin / safepoint / park 协调主线后，当前基线已无法复现先前记录的 `waiting for park: epoch=2 parked=0 need=1` 挂起。
  - `immix_compaction_does_not_move_pinned_objects` 与 `immix_compaction_updates_native_roots_slots_and_object_fields` 单独运行均可稳定通过；整组 `cargo test -p scoop_runtime --test gc_immix_compaction -- --nocapture` 也为绿色。
  - 在此基础上，`cargo test --all` 当前已完整跑通；并对 `cargo test -q -p scoop_runtime --test gc_immix_compaction` 连续复验 20 轮，未再次出现挂起，因此将 `T4007S` 判定为已收口的陈旧验证 blocker，本轮不需要引入额外 runtime 代码补丁。
- 已验证：
  - `cargo test -p scoop_runtime immix_compaction_does_not_move_pinned_objects -- --nocapture`
  - `cargo test -p scoop_runtime immix_compaction_updates_native_roots_slots_and_object_fields -- --nocapture`
  - `cargo test -p scoop_runtime --test gc_immix_compaction -- --nocapture`
  - `cargo test --all`
  - `cargo test -q -p scoop_runtime --test gc_immix_compaction`（连续 20 轮）
  - `cargo clippy --all-targets -- -D warnings`
- 验收：
  - `cargo test -p scoop_runtime immix_compaction_does_not_move_pinned_objects -- --nocapture`
  - `cargo test -p scoop_runtime immix_compaction_updates_native_roots_slots_and_object_fields -- --nocapture`
  - `cargo test --all`
- 依赖：T4007c

### T4007R [DONE] Review：确认 RTTI 不再只覆盖未参数化类型
- 重点：
  - 不允许对 generic / `eff` target 继续静默退回 base unparameterized descriptor / interface id。
  - `dump-rtti` 与运行期 `is/as/as?` 观察到的 canonical name / type_id 必须保持一致。
- 完成：
  - 已复审 `crates/scoopc/src/rtti/mod.rs`、`crates/scoopc/src/rtti/type_desc.rs`、`crates/scoopc/src/itable.rs`、`crates/scoopc/src/llvm/codegen/mod.rs` 与 `crates/scoopc/src/llvm/codegen/gc.rs`：旧 RTTI 查询继续通过 `TypeLowering::new_with_ctx(...)` 解析参数化 nominal，`dump-rtti` 的 class / itable metadata 与 LLVM 运行期 type test 继续统一使用 canonical name + `stable_hash64`，未发现重新静默退回 base unparameterized descriptor / interface id 的旁路。
  - 额外用临时 probe 复验了协变 interface 族的运行期匹配边界：当源码中实际出现 `NamedReadable<Any>` target 时，`dump-rtti --type 'StringReadable'` 的 `runtime_match_type_names` 会同步包含 `NamedReadable<Any>`，并与 `anyValue is NamedReadable<Any>` 的运行结果一致，说明 match set 仍按当前编译单元里的 concrete target 与 assignable 主线统一预计算，而非只保留 base interface id。
  - 未发现新的 RTTI blocker，本轮无需新增生产代码补丁。
- 已验证：
  - `cargo test -p scoopc rtti:: -- --nocapture`
  - `cargo run -q -p scoop -- dump-rtti tests/fixtures/run-pass/type_check_cast_generic_class_instantiation_basic.scoop --type 'Holder<Int>'`
  - `cargo run -q -p scoop -- dump-rtti tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop --type 'StringReadable'`
  - `cargo run -q -p scoop -- dump-rtti tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop --type 'PureManaged'`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/type_check_cast_parameterized_interface_runtime_match_basic.scoop`
  - `cargo run -q -p scoop -- run /tmp/t4007r_named_readable_any_probe.scoop`
  - `cargo run -q -p scoop -- dump-rtti /tmp/t4007r_named_readable_any_probe.scoop --type 'StringReadable'`
  - `cargo run -p scoop -- test`（`fixtures: ok (1055)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4007S

## T4008：effect / continuation 完整性

### T4008 [DONE] 补齐 `Task` 手动 stepping 所需的 effect / continuation 语义缺口（拆分执行）
- 说明：
  - 原任务同时覆盖 escape continuation 多 await 组合、continuation 类型语义 / `resume` builtin surface，以及 richer effect polymorphism / receiver effect op 三条主线，单轮完整收口风险过高。
  - 临时 probe `/tmp/t4008a_single_handle_and_then_probe.scoop` 已确认：单个 setup `handle` 内连续两次 `Async.await` 当前已可稳定 build/run（stdout `17`），因此 `stdlib/task.scoop` 中 `Task.andThen` 的“双层 handle”已经退化为过时 workaround，应优先清理。
  - 为保证每轮只提交一个完整且可验证的切片，现拆分为 `T4008a -> T4008b -> T4008c -> T4008R` 顺序推进。
- 验收：
  - 子任务全部完成后，`ISSUES.md` 第 1 条收窄或关闭。
  - 文档明确区分“已支撑 `Task` manual stepping 的能力”和“仍留待后续的 executor 语义”。
- 依赖：T4007R

### T4008a [DONE] 收口 escape continuation 的多 await 组合，并移除 `Task.andThen` 的双 handle workaround
- 范围：
  - 真实生效的 `stdlib/task.scoop` 不再保留“单个 `handle` 只支持一个 perform 点”的过时注释与嵌套 `handle` 结构。
  - `Task<Int>.andThen` 在单个 setup `handle` 内连续两次 `Async.await`，并继续统一通过 `task.onComplete(executor, k)` 回挂 continuation。
  - 复用或更新回归，覆盖单个 setup `handle` 内链式两次 `Async.await` 的可执行行为。
- 验收：
  - `std_task_async_adapters_basic` 继续通过，且实现不再依赖嵌套 `handle`。
  - 至少一条定向验证直接覆盖“单 handle 两次 await”主线。
- 完成：
  - 真实生效的 `stdlib/task.scoop` 现已把 `Task<Int>.andThen` 收口为单个 setup `handle`：先等待 `this`，拿到 `next` 后立即在同一 `handle` 内继续等待 `next`，outer handler 仍统一通过 `task.onComplete(executor, k)` 回挂 continuation。
  - 已删除真实 stdlib 中“单个 `handle` 只支持一个 perform 点”的过时注释与双 `handle` workaround；`tests/fixtures/typecheck_cone/std_task_async_await_impl_ok/stdlib/task.scoop` 的镜像实现本就已经是单 `handle`，本轮复审确认无需额外修补。
  - 现有 `std_task_async_adapters_basic` run-pass 与临时 probe `/tmp/t4008a_single_handle_and_then_probe.scoop` 已共同覆盖“真实 stdlib `andThen` 行为”与“单 `handle` 连续两次 `Async.await`”两条执行路径。
- 已验证：
  - `cargo run -q -p scoop -- run /tmp/t4008a_single_handle_and_then_probe.scoop`（stdout `17`）
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/std_task_async_adapters_basic.scoop`（stdout 与 golden 一致）
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (349)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4007R

### T4008b [DONE] 收口 continuation 类型语义与 `Continuation.resume` builtin surface（拆分执行）
- 说明：
  - 原任务同时要求 escape continuation binder 的 `Continuation<T, eff E>` 不再退回默认 `Pure`，以及 `Continuation.resume` 的 required-effects / lowering / codegen surface 一并与规范收口。
  - 现有最小 probe `/tmp/t4008b-probe2/continuation_escape_binder_effect_row_is_not_pure.scoop` 已确认：`Ask.current(), k -> { requirePure(k); requireBoom(k) }` 当前会错误成功，说明 binder 仍被注入为 `Continuation<Int, eff Pure>`，而不是能反映恢复后一步语义的 `E`。
  - 进一步检查发现，typecheck 目前只有“整段函数体/handle body 的 performed effects 收集”，并没有“从某个 escape site 恢复后，到下一次 suspension / return / 正常完成”为止的 step-level effect summary。若直接把整段 body 的 effects 塞给 `E`，会把 prefix effects、以及 fresh continuation 之后才执行的 tail 一起误算进去，属于新的规范偏差。
  - 为避免在错误的 effect-row 近似上继续推进，现拆分为 `T4008b1 -> T4008b2`。
- 验收：
  - 子任务完成后，escape continuation binder 与 `Continuation.resume` 都以同一套 resumed-step effect summary 为准。
- 已完成：
  - `T4008b1` / `T4008b2` 已全部完成；escape continuation binder 与 `Continuation.resume` 现统一复用同一份 resumed-step effect summary，不再分别依赖默认 `Pure` 或额外的 builtin 形状猜测。
  - `Continuation.resume` 已按 receiver continuation 的 effect row 做 pure / non-pure 分流：`Continuation<T, eff Pure>.resume(...)` 继续保留 hidden `Raise<RuntimeError>` 语义，`Continuation<T, eff E>.resume(...)` 在 `E` 非 Pure 时走 outward-suspending replay 主线。
- 依赖：T4008a

### T4008b1 [DONE] 将 resumed-step 分析拆分为“tail 重建 / 直接边界总结”与“arm/finally 语义补齐”
- 说明：
  - 继续下钻后确认，原 `T4008b1` 仍同时覆盖两套独立基础设施：
    - 先从 escape site 精确重建 resumed tail（不能把 prefix effects 算进去）；
    - 再把 arm body、`finally`、nested handle、隐藏 init 边界等“step 何时停止、哪些效果属于当前 step”语义全部接上。
  - 若不拆分就直接宣称“已有 resumed-step summary”，很容易把只覆盖直线 direct-perform 场景的近似误当成完整语义，再次形成规范偏差。
  - 现细化为 `T4008b1a -> T4008b1b`：先固定可复用的 resumed-tail 重建与 direct boundary summary 主线，再补齐复杂边界语义。
- 依赖：T4008a

### T4008b1a [DONE] 为 escape site 重建 resumed tail，并收口 direct perform / direct call 的 step effect summary
- 范围：
  - 为 handle body 中命中的 escape site 重建“从该 site 恢复后”的 resumed tail，不能把 prefix effects 算进去。
  - 基于该 tail 为 direct `perform` / direct effectful call 建立 step-level effect summary；再次命中 escape continuation 边界时必须停止，不能把 fresh continuation 之后的 tail 算进去。
  - 补充最小回归，至少覆盖“后续 tail 含 `Boom` 时，当前 site 的 summary 不再是 `Pure`”以及“第二个 escape site 之后的 tail 不计入第一个 site”两类场景。
- 验收：
  - 有稳定的内部分析 API 或等价结果，能按 escape continuation binder 暴露 direct resumed-step 的 `EffectRow`。
  - 新回归能够复现并卡住当前 `k` 被错误注入为 `eff Pure` 的根因样例。
- 已完成：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 现新增 `compute_escape_continuation_direct_step_effect_rows_for_handle(...)` 及相关 resumed-tail / direct-step summary 辅助入口，复用现有 `HandlePlanBuilder` 与 ordinary-callee resume-tail 重建逻辑，从命中的 escape site 精确重建“恢复后的一步”。
  - direct summary 现覆盖 direct `perform` 与 handle 内局部函数值的 direct effectful call；再次命中 escape continuation 边界时会停止，不再把后续 site 的 tail 误算进当前 step。为避免只依赖 HIR `callee.ty`，analysis 还会回退读取 handle 内局部声明元数据里的函数值类型。
  - 已新增 2 条 Rust 单测，分别覆盖“resumed tail 中的 direct effectful function-value call 会被计入 summary”与“第二个 escape site 之后的 tail 不计入第一个 site”。
- 已验证：
  - `cargo test -p scoopc direct_step_`
  - `cargo test --all`
  - `cargo run -p scoop -- test`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4008a

### T4008b1b [DONE] 为 resumed-step 补齐 arm body / `finally` / nested handle / hidden boundary 语义
- 范围：
  - direct summary 不再把 non-resuming / immediate-resume arm body、`finally`、nested handle、顶层/对象 once-init 等隐藏边界遗漏在外。
  - current handle inactive/active 切换要与现有 state-machine 语义一致，不能把 arm body 中由外层处理的效果重新算成当前 handle 已处理。
  - 补充对应回归，覆盖 direct summary 之外的复杂边界语义。
- 验收：
  - `T4008b1a` 提供的 API 对上述边界场景也给出与 state-machine 语义一致的 step effect row。
- 已完成：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 的 resumed-step summary 现已区分 current-handle active body 与 arm/finally/once-init 等 inactive 区域：active 边界产生的 effect 会重新走当前 handle 的 dispatch 语义，而 arm body / `finally` 中产生的 effect 不会被错误地回算成“仍由当前 handle 处理”。
  - escape continuation direct-step API 现支持 arm body、`finally`、nested handle boundary 与 hidden once-init boundary 四类复杂路径；hidden boundary 新增 program side table 输入，可把顶层 immutable value 与 object init 的一次初始化步骤纳入同一份 summary。
  - 已新增 5 条 Rust 单测，分别覆盖 immediate-resume arm body、下一次 escape arm body、`finally`、nested handle boundary 与 hidden top-level once-init effect row；原有 2 条 `T4008b1a` 单测继续保持通过。
- 已验证：
  - `cargo test -p scoopc direct_step_`
  - `cargo test --all`
  - `cargo run -p scoop -- test`（`fixtures: ok (1055)`）
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4008b1a

### T4008b2 [DONE] 基于 resumed-step summary 收口 continuation binder 类型与 `Continuation.resume`
- 范围：
  - `, k ->` 注入 `Continuation<T, eff E>` 时使用 `T4008b1` 计算出的 step-level `E`，不再退回默认 `Pure`。
  - `Continuation.resume` 的 typecheck / lowering / codegen 与规范 `k.resume(value): Unit / (E + Raise<RuntimeError>)` 对齐，复用同一份 resumed-step 语义，不再保留额外的 shape-based 调用限制。
  - 补充 typecheck / run-pass 回归，覆盖 binder 类型区分、effect-row 传播与 `resume` 调用主线。
- 验收：
  - binder 类型不再能被 `requirePure(k)` 误接收。
  - `resume` required-effects 会从推导出的 `E` 正确传播。
- 已完成：
  - `check_file_exprs` 现已改为两阶段：第一阶段先完成常规 expr typecheck 并收集 typechecked side tables，随后基于 `T4008b1` 的 resumed-step summary 重新计算 escape continuation binder 的精确 effect row，再用第二阶段把 `, k ->` 注入类型收口为 `Continuation<T, eff E>`，不再默认退回 `Pure`。
  - `Continuation.resume` 的 typecheck / lowering / LLVM effect segmentation 现统一消费同一套 side tables：AST / HIR 会同时记录“所有 `Continuation.resume` 调用点”与“non-pure continuation resume 调用点”，pure continuation 继续分类为 hidden `RuntimeRaise`，non-pure continuation 才分类为 `CallMaySuspend` 并进入 replay 主线。
  - runtime / codegen 已补齐 resumed tail 再次 outward suspend 时的 replay 状态传递：outer `handle { k.resume(...) }` 现在可以继续接住 resumed tail 中后续产生的 effect，而 pure continuation 的嵌套 `try/catch` / `handle` 语义不再被错误破坏。
  - 已新增回归：
    - `tests/fixtures/typecheck/continuation_escape_binder_effect_row_is_not_pure.scoop`
    - `tests/fixtures/typecheck/continuation_resume_from_escape_binder_requires_step_effect.scoop`
    - `tests/fixtures/run-pass/continuation_escape_binder_resume_effect_row_runtime_basic.scoop`
    - `crates/scoopc/src/typecheck/expr/entry.rs` 中 3 条 binder/retype 单测
    - `crates/scoopc/src/llvm/codegen/effect/state_machine_segments.rs` / `state_machine_transform.rs` / `state_machine_emitter.rs` 中 pure/non-pure continuation resume 分类与 replay 回归
- 已验证：
  - `cargo test -p scoopc continuation_resume -- --nocapture`
  - `cargo test -p scoopc when_arm_try_resume_nested_handle_ir_keeps_binder_scope_for_inner_resume -- --nocapture`
  - `cargo run -q -p scoop -- test --fixtures target/t4008b2-fixtures/infer`（`fixtures: ok (2)`）
  - `cargo run -q -p scoop -- test --fixtures target/t4008b2-fixtures/run-pass`（`fixtures: ok (1)`）
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (331)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4008b1b

### T4008c [DONE] 将 richer effect polymorphism 与 receiver effect op lowering 拆分执行
- 说明：
  - 原任务同时要求收口“多 effect type params 的 effect instance 实例化 / handler 匹配”与“带 receiver 的 effect op 调用 / handle lowering / state-machine codegen”，横跨 typecheck、HIR lowering 与 LLVM effect dispatch 三层。
  - 继续审计 `crates/scoopc/src/typecheck/expr/call.rs` 与 `crates/scoopc/src/typecheck/expr/infer.rs` 后确认，现有缺口也体现在两类独立 hard gate：一类是 `effect_sym.type_param_names.len() > 1` 直接 early reject，另一类是 `op.sig.receiver.is_some()` 直接报 unsupported。
  - 为避免单轮同时放大“effect instance 实例化”和“receiver 参与 perform payload / arm binder 布局”两套回归矩阵，现细化为 `T4008c1 -> T4008c2` 顺序推进。
- 依赖：T4008b2

### T4008c0 [DONE] 修复 statement-position if/else mixed replay 在 else 分支丢失第二次 continuation
- 说明：
  - 在当前工作树和 clean `HEAD`（`45a1144`）里，`tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_if_multi.scoop` 都会错误输出 `fetch_resume / resume_else_1 / missing2`，而不是 golden 中的 `fetch_enter 40 / ask_arm 2 / 41 / resume_else_2`。
  - 这说明无 immediate-resume 的 multi-arm handle 在 statement-position if/else 中处理“direct perform 之后再命中同分支 indirect site”的 replay 时，else 分支仍会把 indirect site 错误重放成 direct site 的 resumed tail，导致 fresh continuation 丢失。
- 范围：
  - 修复 else 分支在 direct + indirect same-stmt mixed 场景下的 source-path / escape replay 目标构造。
  - 补充对应 run-pass / state-machine regression，锁定第二次 continuation 的重放与 `after_resume1` / `after_resume2` 顺序。
- 完成：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 现按 suspend 场景细分 `pending continuation` 的发布时机：call-like / unmatched outward perform 仍在 `Suspend` 处发布，`EscapeContinuation` arm 改为在 `ArmMaterializeContinuation` 处发布，避免 non-resuming / immediate-resume arm 污染 replay-state。
  - `runtime/c/scoop_runtime.c` 保留了 callee suspend TLS/replay-state 不被错误 resurrect 的修正，并移除了临时调试日志；新增 emitter IR 回归与 runtime 集成测试，同时补齐 test-only runtime ABI allowlist。
- 验收：
  - `tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_if_multi.scoop` 重新与现有 golden 对齐。
  - `cargo run -p scoop -- test` 不再被该 fixture 阻塞。
  - 已验证 `cargo test -q -p scoopc non_resuming_arm_ir_does_not_publish_pending_continuation -- --nocapture`、`cargo test -q -p scoop_runtime continuation_resume_preserves_step_fn_replaced_callee_suspend_state -- --nocapture`、`cargo test -q -p scoop_runtime continuation_resume_does_not_resurrect_saved_replay_state_tls -- --nocapture`、`cargo run -q -p scoop -- run tests/fixtures/run-pass/effect_multi_escape_custom_nonresuming_direct_indirect_if_multi.scoop`、`cargo run -q -p scoop -- test`（`fixtures: ok (1058)`）、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
- 依赖：T4008c

### T4008c1 [DONE] 收口多 effect type params 的 effect op call / handle arm 实例化
- 范围：
  - effect op call / handle arm 不再硬编码拒绝 2+ effect type params。
  - performed-effect / handled-effect side table 保留完整 effect type args，不再只按单一尾参数回填 effect instance。
  - 补充对应 typecheck / run-pass 回归，覆盖多 effect type params 的调用、handler 捕获与 effect instance dispatch；回归不再依赖尚未收口的“多实参 perform payload”路径。
- 验收：
  - `effect Pair<K, V>` 一类 effect 的 op call 与 handle arm 能进入统一 typecheck / lowering / runtime dispatch 主线。
  - `ISSUES.md` 第 1 条中“effect type param 仍只支持单一 type param”的部分收窄或关闭。
- 完成：
  - `infer_effect_op_call_expr_type` 与 `lower_handle_arm_effect_op_sig` 不再对 `effect_sym.type_param_names.len() > 1` 早退报错；effect op 可实例化签名现统一纳入“全部 op type params + 全部 effect type params”，不再把 effect instance 退化成单一尾参数。
  - performed-effect / handled-effect side table 现会保留完整 effect type args：effect op call 会把完整实例化后的 effect type args 回填到 `record_inferred_performed_effect_ty`，handler arm 则可通过 binder 类型注解或 body 内 performed effect 的唯一候选反推出完整 handled effect，并写回 `record_inferred_handle_arm_effect_ty` 供 HIR lowering / LLVM dispatch 复用。
  - 已新增回归：
    - `tests/fixtures/typecheck/effect_multi_type_params_tuple_payload_ok.scoop`
    - `tests/fixtures/run-pass/effect_multi_type_params_dispatch_basic.scoop`
    - `tests/fixtures/run-pass/effect_multi_type_params_dispatch_basic.stdout`
- 已验证：
  - `cargo fmt --check`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/effect_multi_type_params_dispatch_basic.scoop`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (332)`）
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1060)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4008c0

### T4008cP [DONE] 收口普通 `perform` lowering 的多实参 payload transport
- 说明：
  - 在为 `T4008cS` 先做最小 probe 时发现，问题不只出在 state-machine：`crates/scoopc/src/llvm/codegen/effect/mod.rs::codegen_perform_expr` 当前同样只消费第 0 个实参。
  - 最小探针 `fun go(): Int / Edge { return Edge.visit(3, 4) }` + 外层 `handle { go() } with { Edge.visit(from, to) -> ... }` 当前实际输出 `3 / 3` 并返回 `6`，说明第二个 payload 已在普通 callee 的 perform lowering 上被静默丢失。
  - 如果只修 `T4008cS`，那么 handle body 内 direct perform 虽可通过，但“普通 effectful callee -> 外层 handle 捕获”这条主线仍会继续错误折叠多实参 payload；因此必须先把共享的 perform transport 合同收口，再继续 state-machine 专用路径。
- 范围：
  - 普通 `perform` lowering 支持 2+ 实参 effect op，不再只把第 0 个实参写入 perform slot。
  - 与现有 handler binder 读取顺序对齐，保证多 binder payload 在 indirect perform / outer handle 捕获路径下按源码顺序可见。
  - 补充对应 run-pass / LLVM regression，覆盖“普通 effectful callee perform 两个 payload，外层 handle 读取两个 binder”。
- 验收：
  - 上述最小 probe 不再输出 `3 / 3`，而是稳定得到 `3 / 4` 与返回值 `7`。
  - 为后续 `T4008cS` / `T4008c2` 提供统一的多 payload transport 合同。
- 已完成：
  - typecheck 现会为 effect-op 调用记录 `arg_mapping` side table；HIR lowering 继续把它收口为 `EffectOpCallInfo { arg_mapping, payload_tuple_ty }`，并为多 binder handler arm 额外记录 `handle_payload_tuple_tys`，避免 LLVM 再按源码形状猜测 payload 布局。
  - 普通 `perform` lowering 现对 2+ 实参 effect op 统一走“按源码顺序求值显式实参，再按形参顺序打包成 tuple transport value”的主线；多 payload 会通过既有 `EffectValueBox` 共享 transport ABI 写入 perform slot，不再静默丢掉第 1 个之后的实参。
  - handler arm 多 binder 读取现会按 `handle_payload_tuple_tys` 一次性解码整组 transported tuple，再按 binder 顺序投影各元素；因此 ordinary callee `perform` 被外层 `handle` 捕获时，多 binder 可稳定看到完整 payload。
  - 已新增回归：
    - `tests/fixtures/run-pass/effect_indirect_multi_payload_transport_basic.scoop`
    - `tests/fixtures/run-pass/effect_indirect_multi_payload_transport_basic.stdout`
    - `crates/scoopc/src/llvm/mod.rs` 中的 `indirect_multi_payload_perform_boxes_and_unboxes_tuple_transport`
- 已验证：
  - 最小 probe `fun go(): Int / Edge { return Edge.visit(3, 4) }` + 外层 `handle { go() } ...` 实际输出 `3`、`4`，进程退出码为 `7`
  - `cargo test -p scoopc indirect_multi_payload_perform_boxes_and_unboxes_tuple_transport`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/effect_indirect_multi_payload_transport_basic.scoop`（stdout `left / 6`，退出码 `10`）
  - `cargo fmt --check`
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1061)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4008c1

### T4008cS [DONE] 支持 effect state-machine 的多实参 perform payload lowering
- 说明：
  - 在为 `T4008c1` 增加 run-pass 回归时发现，`crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs::emit_perform_op` 当前只接受 0/1 个 payload arg，`effect Edge<A, B> { fun visit(from: A, to: B): Int }` 一类两形参 effect op 在 state-machine 路径会报 `state machine perform arity`。
  - 在实际 probe 中又确认，state-machine 之前还存在更前置的普通 `perform` lowering 缺口：多实参 indirect perform 会先在普通 callee 上丢掉额外 payload，因此本任务现顺延到 `T4008cP` 之后，只负责 handle state-machine 自身的 payload / binder 主线。
  - 如果不先收口，后续 receiver effect op 把 receiver 当作第 0 个 payload 后会继续撞上同一限制。
- 范围：
  - state-machine perform / handler dispatch 支持 2+ 形参 effect op，不再只接受单 payload expr。
  - 补充对应 run-pass / LLVM regression，覆盖同一 effect op 的多 binder payload 进入 perform slot 与 arm binder 读取。
- 验收：
  - 多形参 effect op 在普通 handle / immediate-resume / escape-continuation 路径都不再报 `state machine perform arity`。
- 完成情况：
  - `crates/scoopc/src/llvm/codegen/effect/state_machine_emitter.rs` 的 direct perform lowering 现已复用普通 `perform` 的共享 tuple payload transport，不再把 state-machine 路径限制在 0/1 payload。
  - 在继续复验时，一并修复了 `crates/scoopc/src/llvm/codegen/effect/state_machine_plan.rs` 里更前置的 resume rewrite 缺口：当 direct `perform` 位于 `if` 表达式内部，且 outer consumer 落在后继 merge state / goto 链共享 state 中时，resume 路径现在会克隆并改写专用 consumer state，避免把 resume slot 污染到普通分支，也不再把恢复值错误回退成 `0`。
  - 新增 run-pass `tests/fixtures/run-pass/effect_state_machine_multi_payload_basic.scoop`，覆盖多 payload direct perform 在 plain / immediate-resume / escape-continuation 三条 state-machine 路径下的 transport 与恢复值传递。
  - 新增 source-plan 回归 `source_plan_clones_if_expr_merge_consumer_for_resume_path`，以及 LLVM 回归 `state_machine_multi_payload_perform_uses_tuple_transport`。
- 已验证：
  - 最小 probe `/tmp/t4008cs_probe3.scoop` 现打印 `7`，退出码为 `8`
  - `cargo test -p scoopc source_plan_clones_if_expr_merge_consumer_for_resume_path -- --nocapture`
  - `cargo test -p scoopc state_machine_multi_payload_perform_uses_tuple_transport -- --nocapture`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/effect_state_machine_multi_payload_basic.scoop`（stdout 恢复为预期，退出码 `177`）
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/effect_resume_if_then_branch_single_perform.scoop`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/effect_resume_if_else_branch_single_perform.scoop`
  - `cargo fmt --check`
  - `cargo test --all`
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1062)`）
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4008cP

### T4008c2 [DONE] 打通 receiver effect op 的 perform / handler lowering / codegen
- 范围：
  - receiver effect op 不再在 typecheck 早退报 unsupported。
  - receiver effect op 的调用与 handle arm binder 统一按“receiver 作为第 0 个形参 / 第 0 个 binder”进入 HIR / LLVM effect dispatch 主线。
  - 补充 parse / typecheck / run-pass 回归，避免把 receiver effect op 做成 ad-hoc 特判。
- 验收：
  - `ISSUES.md` 第 1 条中关于 receiver effect op 的剩余描述收窄或关闭。
- 已完成：
  - `infer_effect_op_call_expr_type` 与 `lower_handle_arm_effect_op_sig` 不再对 `op.sig.receiver.is_some()` 早退报 unsupported；receiver effect op 现统一把 receiver 显式降为第 0 个形参，因此命名/位置实参与 effect-instance 推断继续复用既有 `arg_mapping` / instantiation 主线，而不是新开 receiver 专用分支。
  - handler arm 的可实例化 effect-op 签名现同样把 receiver 降为第 0 个 binder；binder arity、类型注解校验、handled-effect 推断与 side table 写回都复用现有多 binder 主线，不再把 receiver effect op 当成另一套 arm 语义。
  - HIR / LLVM 未新增 receiver 专用 lowering：receiver effect op 直接复用现有 `Perform.args`、`EffectOpCallInfo { arg_mapping, payload_tuple_ty }`、`handle_payload_tuple_tys` 与 ordinary/state-machine perform transport，因此“receiver + 普通参数”的 2-payload effect op 已能在 non-resuming / immediate-resume / escape-continuation handler 中稳定执行。
  - 已新增回归：
    - `tests/fixtures/parse/effect_op_receiver_decl_basic.scoop`
    - `tests/fixtures/parse/effect_op_receiver_decl_basic.ast`
    - `tests/fixtures/typecheck/effect_receiver_op_call_and_handle_ok.scoop`
    - `tests/fixtures/run-pass/effect_receiver_op_basic.scoop`
    - `tests/fixtures/run-pass/effect_receiver_op_basic.stdout`
- 已验证：
  - `cargo fmt --check`
  - `cargo run -q -p scoop -- dump-ast tests/fixtures/parse/effect_op_receiver_decl_basic.scoop`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/effect_receiver_op_basic.scoop`（stdout 符合预期，退出码 `30`）
  - `cargo test --all`
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1065)`）
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4008cS

### T4008c3 [DONE] 收口 handler arm head 的 effect-op 绑定主线
- 范围：
  - handler arm head 与 effect op call / perform 共用同一套 callee 绑定与 effect-instance 实例化语义，不再只接受裸 effect-op 形状。
  - 多 effect type params、receiver effect op 与多 payload binder 在 arm head 上统一进入同一 side table / lowering 主线。
  - 补充对应 parse / typecheck / run-pass regression，覆盖 arm head 不再依赖早期 AST 形状才能匹配 handler。
- 验收：
  - `ISSUES.md` 第 1 条中“handler arm head 仍只接受 effect operation”的部分收窄或关闭。
- 已完成：
  - `crates/scoopc/src/parser/expr.rs` 的 `parse_handle_op` 现同时支持 `Effect<T>.op(...)` 与 `Effect.op<U>(...)` 两种 handler arm head 形态；`looks_like_handle_arm_start_at` 也同步识别带 `<...>` 的 arm head，因此 generic arm head 不再因为 early shape gate 被当成普通表达式，parse recovery 也不会误吞后续 `->`。
  - `crates/scoopc/src/ast/mod.rs` 的 `HandleOp` 新增 `op_type_args`，`crates/scoopc/src/typecheck/expr/call.rs` 则抽出 `lower_effect_op_signature` 供 effect-op call 与 handler arm 共享；`crates/scoopc/src/typecheck/expr/infer.rs` 不再单独拼接 arm 签名，而是直接复用共享 helper 来处理 effect type args、op type args、receiver 与多 payload binder，并把 handled-effect 实例继续写回同一 side table。
  - `crates/scoopc/src/hir/lower/expr.rs` 现把 generic effect-op call 的 `TypeApply(callee)` 视为透明包装，因此 `Query.ask<Int>(...)` 这类显式 op type args 调用在 lowering 时仍会进入 `Perform` 主线，不再退回普通 `Call(TypeApply(...))` 分叉。
  - 已新增回归：
    - `tests/fixtures/parse/handle_arm_explicit_type_args_basic.scoop`
    - `tests/fixtures/parse/handle_arm_explicit_type_args_basic.ast`
    - `tests/fixtures/typecheck/handle_arm_explicit_type_args_ok.scoop`
    - `tests/fixtures/run-pass/handle_arm_explicit_type_args_basic.scoop`
    - `tests/fixtures/run-pass/handle_arm_explicit_type_args_basic.stdout`
- 已验证：
  - `cargo fmt --check`
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1068)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4008c2

### T4008c4 [DONE] 扩展 `Continuation.resume` 的调用 surface，并收口 `ImmediateResume` storage contract
- 说明：
  - `T4008b2` 已收口 continuation binder 的 effect-row 与 replay 主线，但最新 `ISSUES.md` 第 1 条仍保留两项未完成 surface：`Continuation.resume` 目前仍只接受一个实参、命名实参也只认 `value = ...`；同时 `-> resume` / `ImmediateResume` 对应的 stack-local fast path 仍未形成独立 storage 选择，heap-only full machine 只是当前保守实现。
  - 这两点都不适合继续隐含在 review 里带过，需要显式任务来决定“真正实现”还是“明确写成 deferred contract”。
- 范围：
  - `Continuation.resume` 进入统一 callable-value binder：不再只支持单值 payload，也不再只接受 `value = ...` 这一种命名实参。
  - receiver / named args / payload arity 与普通调用语义保持一致，避免 continuation resume 再保留单独的 shape-based surface。
  - `ImmediateResume` / `-> resume` 的 lowering contract 明确化：若本轮仍保持 heap-only full machine，则必须把 stack-local fast path 记为显式 deferred item，并把 `SCOOP_FULL_SPEC.md` / `SCOOP_RUNTIME.md` / 相关 sysroot 注释的同步任务纳入验收，而不是继续保留隐含承诺。
- 验收：
  - `ISSUES.md` 第 1 条中关于 `Continuation.resume` surface 的剩余描述收窄或关闭。
  - `ImmediateResume` 若仍未实现 stack-local storage，也必须留下明确的 deferred contract 与文档同步任务。
- 完成：
  - `Continuation.resume` 的 typecheck 现已从“硬编码单参数 + 仅 `value = ...`”扩展为共享 payload-shape surface：保留旧的单 payload `k.resume(value)` 兼容路径，同时新增 `Continuation<Unit>.resume()` 与 tuple payload 的扁平 `k.resume(v0, v1, ...)` / `k.resume(a0 = ..., a1 = ...)` 调用面；required-effects / `Raise<RuntimeError>` / call-site side table 仍复用原有 continuation resume 主线，没有新开第四套 effect 语义。
  - LLVM codegen 现会在拿到 receiver 精确 `Continuation<T>` 类型时，按同一 payload-shape 规则组装 resume payload：`Unit` 走零参、tuple 走元素级重排并重新打包成 tuple value，再复用既有 `(resume_word, resume_gc_ref)` transport；当后端暂时拿不到 receiver 精确类型时，则保守回退到旧的单 payload lowering，避免把既有 generator / scheduler fixture 回归打断。
  - 在收口过程中发现并修复了一个由新 surface 暴露出来的既有裂缝：escape continuation arm 在 state-machine emitter 里会把 continuation binder 重新写入 env，但原实现把精确 `hir_ty` 抹成了 `None`，导致 generator 这类旧路径无法恢复 `Continuation<T>` payload type。现已保留该 binder 的精确 `hir_ty` 与 `call_may_suspend`，`generator_yield_iter_int_basic` 再次恢复绿色。
  - `ImmediateResume` / `-> resume` 本轮没有引入新的 runtime storage 表示；相反，已把“当前编译器仍统一使用 GC-managed full machine，stack-local fast path 只是 deferred optimization”同步写入 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md` 与 `sysroot/core.scoop` 注释，移除先前的隐含承诺。
  - 已新增回归：
    - `tests/fixtures/typecheck/continuation_resume_surface_ok.scoop`
    - `tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.scoop`
    - `tests/fixtures/run-pass/continuation_resume_surface_named_tuple_and_unit_basic.stdout`
- 依赖：T4008c3

### T4008R [DONE] Review：确认 effect 完整性收口没有引入新的 shape-based lowering
- 重点：
  - continuation / effect codegen 不能回流到按源码形状补丁选路。
  - handler arm head 与 `Continuation.resume` 不能继续保留独立于普通 call binder 的 surface 特判。
  - 若 `ImmediateResume` 仍维持 heap-only lowering，review 结论里必须明确记录其 deferred contract，而不是静默保留 spec/实现分裂。
- 完成：
  - 已复审 `crates/scoopc/src/typecheck/expr/call.rs::lower_effect_op_signature`、`crates/scoopc/src/typecheck/expr/infer.rs::lower_handle_arm_effect_op_sig` 与 `crates/scoopc/src/hir/lower/expr.rs::maybe_lower_effect_op_call`：effect-op call、receiver effect op 与 handler arm head 继续共享同一套签名 lowering、`arg_mapping` 与 payload tuple metadata；HIR `EffectOpCallInfo` 记录的是 typecheck 已确认的 `arg_mapping` / `payload_tuple_ty`，LLVM `codegen_perform_payload_value` 与 state-machine arm binder 只消费这份 side table，没有按 binder 数量、receiver 形态或源码写法重新猜 transport。
  - 已复审 `try_infer_continuation_resume_call_expr_type`、`continuation_resume_call_sites`、`codegen_continuation_resume_builtin` 与 `classify_builtin_suspend_call`：`Continuation.resume` 的 surface 继续复用普通 call binder helper（`collect_call_arg_infos`、命名实参校验、`map_call_args_to_params` / `map_call_args_to_params_by_name`），state-machine / LLVM 是否把调用识别为 builtin 只看 typecheck 写回的 call-site side table，不再按 member 名、receiver 形状或 payload 个数做语义选路；codegen 内部的 `callee shape` 检查只是对已选 builtin 路径的结构断言。
  - 已复审 `SCOOP_FULL_SPEC.md`、`SCOOP_RUNTIME.md` 与 `sysroot/core.scoop`：`ImmediateResume` / `-> resume` 当前仍统一走 GC-managed full-machine lowering，stack-local fast path 已明确记录为 deferred optimization，不再保留静默的 spec/实现分裂。
- 已验证：
  - `cargo run -q -p scoop_tools -- spec-fixtures check`
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1070)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4008c4

## T4009：`Task` 设计定型

### T4009 [TODO] 收口 `Task<T>` core，并移除当前代码库中 executor-centric 实现（拆分执行）
- 说明：
  - 最新设计讨论已明确：`Task<T>` 应是普通 Scoop class，`async {}` 与 `async fun` 只是 `Task` sugar；语言合同不要求 task-specific special codegen。
  - 当前 runtime / sysroot / stdlib / LLVM 中的 executor-centric implementation 与 special-case 已被确认是 premature / incomplete / wrong direction。`T4009a [DONE]` 只保留“脱离旧 handle ABI”的历史清理记录，不再代表 `Executor` / `spawn` / runtime executor 方向应继续扩展。
  - 因此本组任务重排为 `T4009a1 -> T4009a2 -> T4009a3 -> T4009b -> T4009c -> T4009h -> T4009R`：先统一 async surface，再移除 executor-centric surface 与 special-case，再固定 `Poll<T>` / `Task.poll()` / private suspended-state carrier，把 `spawn` 明确移出当前 core，并补齐 stable handle 作为 wake token 的合同。
- 验收：
  - 子任务全部完成后，`ISSUES.md` 第 2 条收窄或关闭。
  - `SCOOP_FULL_SPEC.md` 对 `Task` / `Continuation` / async surface 的边界表述一致。
  - 如 runtime / sysroot 合同改变，相关文档同步更新。
- 依赖：T4008R

### T4009a [DONE] 拆掉 `Task<T>` / `Executor` 的 hard-coded handle ABI 绑定
- 范围：
  - `spawn { ... }` body 不再被旧 ABI 绑死为 `Int`；`Task<T>` / `Executor` 也不再直接映射成 word-sized runtime handle。
  - `taskCreate` / `onComplete` / `join` 的 lowering 不再把旧 runtime symbol（如 `scoop_task_u64_create`、`scoop_task_u64_on_complete_resume_u64`、`__scoop_task_join_int`）当成 `Task` 公开语义本体。
  - 为后续普通对象表示补齐必要的 typecheck / HIR / LLVM / runtime metadata，使 `Task` 能从“hard-coded ABI handle”过渡到“有明确对象模型的值”。
- 验收：
  - `Task` / `Executor` 的语言语义不再依赖旧 handle ABI 叙事才能成立。
  - `spawn` / `join` / `onComplete` 的 regression 能覆盖“非 `Int` body”“非 handle 直通”与 object-model 相关路径。
- 依赖：T4009

### T4009a1 [DONE] 统一 `async {}` / `async fun` 为 `Task<T>` sugar
- 范围：
  - `async { body }` 的 typecheck / HIR / lowering / 文档统一收口为 lazy `Task<T>` surface，不再保留“吞掉内部 `Async` effect 并直接返回 `T`”的旁路叙事。
  - `async fun foo(): T` 与 `async { ... }` 共享同一 `Task` desugar 主线；`Task<T>` 作为普通 class 成立，不引入 task-specific special codegen requirement。
  - 如 `Task` 内部需要 suspended-state / continuation carrier，应保持为私有实现细节，不继续把 raw `Continuation` 暴露成默认 async API。
- 验收：
  - `async {}` / `async fun` 对调用者都稳定暴露 `Task<T>`，且叙事与 lowering 主线一致。
  - `SCOOP_FULL_SPEC.md` / `TODO.md` / 相关 sysroot 文档对 async surface 的表述不再分裂。
- 完成：
  - `crates/scoopc/src/typecheck/expr/infer.rs` 现将 `async { ... }` 的结果类型统一定为 `Task<T>`，并继续保留“`await` 仅能在 async 语境内等待 `Task<T>`”的静态规则。
  - `crates/scoopc/src/hir/lower/mod.rs`、`crates/scoopc/src/hir/lower/block.rs` 与 `crates/scoopc/src/hir/lower/expr.rs` 已把 `async { ... }` 与 `async fun` 统一 lower 到同一条 lazy `taskCreate { ... }` 主线；task body 内部的 `await expr` 会改写为内部 `__scoop_task_join(expr)`，不再继续走旧的 hidden `Async.await` immediate-resume 路径。
  - `runtime/c/scoop_task_executor.c` 现为最小 lazy task 链补齐执行合同：`scoop_task_join` 在遇到 `CREATED` 且带 body 的 task 时会同步直驱一次 body，再读取结果，从而让 `await fetch()` / `await async { ... }` 这类路径与 `async fun` 共享同一语义。
  - `sysroot/core.scoop` 已同步注明当前 async surface 的最小合同；新增回归覆盖 typecheck、LLVM IR、runtime test 与 run-pass：
    - `crates/scoop_runtime/tests/task_spawn_join.rs`
    - `tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop`
    - `tests/fixtures/run-pass/async_fun_task_runtime_basic.stdout`
    - 以及更新后的 `async_await_minimal_int_basic` / `spawn_join_*` / `async_fun_returns_task_ok` / `entry_point_main_spawn_join_async_ok` 等夹具。
- 已验证：
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/async_await_minimal_int_basic.scoop`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/spawn_join_int_basic.scoop`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/spawn_join_string_basic.scoop`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop`
  - `cargo test -q -p scoopc async_task_ir_uses_task_create_and_internal_join -- --nocapture`
  - `cargo test -q -p scoop_runtime --test task_spawn_join task_join_runs_created_task_body_inline -- --nocapture`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
  - `cargo run -q -p scoop -- test`
- 依赖：T4009a

### T4009a2 [DONE] 从 core / runtime / sysroot / stdlib 移除当前 executor-centric `Task` surface
- 范围：
  - 当前 `Executor` 类型、executor-centric `Task` API、以及围绕它们组织的 stdlib / sysroot surface 不再作为当前 `Task` core 的公开合同。
  - 当前不完整的 runtime executor implementation（例如 `runtime/c/scoop_task_executor.c`）从代码库主线中移除；保留的仅是 GC、effect/continuation runtime、stable handle 等通用能力。
  - 如需保留 future executor 的方向，只能以 deferred notes 记录，不能继续以现有 surface / symbol 形式留在 core 叙事里。
- 验收：
  - 当前代码库不再把 executor surface 当作 `Task` 语义本体的一部分。
  - 相关文档、注释与 sysroot surface 中不再把当前 executor implementation 描述成推荐方向。
- 完成：
  - 已删除 `sysroot/task.scoop` / `stdlib/task.scoop` 以及围绕 `Executor`、`taskCreate*`、`tryStart`、`onComplete`、`map/andThen/await` 组织的公开 surface；`sysroot/core.scoop` 现仅保留 `Task<T>` / `Async` 与内部 `__scoop_task_create` / `__scoop_task_from_result` / `__scoop_task_join` helper。
  - 已将 runtime 的 task 实现从 `runtime/c/scoop_task_executor.c` 拆成 executor-free 的 `runtime/c/scoop_task.c`，只保留 lazy create、completed-from-result 与 synchronous join 三条最小 `Task` core 路径，并同步收缩 `runtime/c/scoop_runtime_api.h` allowlist 与 `crates/scoop_runtime/build.rs` 编译清单。
  - `async {}` / `async fun` lowering 现改用内部 FQN `scoop.core.__scoop_task_create`；受影响的 run-pass / typecheck / cone 夹具已统一改为通过 handled `Async.await(...) -> __scoop_task_join(...)` 驱动外层 lazy task，不再依赖公开 executor surface。
  - 收口过程中发现并修复了一个既有 GC 裂缝：RTTI trace bitmap 仍把 `Task` 当作 early-stage handle，导致“`Task` 仅通过对象字段可达”时可能漏扫；现已恢复为普通 GC ref，并新增 `gc_trace_task_field_basic` 回归覆盖该路径。
  - `SCOOP_FULL_SPEC.md` / `ISSUES.md` / `STDLIB_COMPLETENESS.md` / `sysroot/core.scoop` 已同步改写为“当前阶段无公开 `scoop.task` executor package”的叙事。
- 已验证：
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (334)`）
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (357)`）
  - `cargo test -q -p scoopc async_task_ir_uses_task_create_and_internal_join -- --nocapture`
  - `cargo test -q -p scoop_runtime --test task_spawn_join`
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1070)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4009a1

### T4009a3 [DONE] 移除 LLVM / runtime 对当前 executor / `scoop.task.*` 的 hard-coded special-case
- 范围：
  - LLVM codegen、runtime ABI 与相关 lowering 中，凡是只为当前 executor-centric 方向存在的 `scoop.task.*` / `Executor` / task-runtime special-case 都要移除。
  - `Task<T>` core 语义应建立在普通对象 / class lowering 与既有 effect / continuation 主线上，而不是继续依赖专门的 executor ABI 分叉。
  - 保留的 runtime 支撑只限于通用 continuation、GC、stable handle、必要的 effect transport 等基础能力。
- 验收：
  - `Task` core 不再依赖当前 executor 专用 ABI / symbol / codegen 分支才能成立。
  - 任何剩余的 future executor 钩子都被明确标记为 deferred，而不是当前合同的一部分。
- 完成：
  - `crates/scoopc/src/llvm/codegen/mod.rs` 已删除对 `scoop.task.executorCreate`、`scoop.task.destroy`、`scoop.task.debugPendingCount`、`scoop.task.runNext`、`scoop.task.runUntilIdle`、`scoop.task.taskCreateManual`、`scoop.task.state`、`scoop.task.result`、`scoop.task.tryStart`、`scoop.task.complete`、`scoop.task.onComplete` 的 FQN 分发与对应 helper；当前 compiler 仅保留 async sugar 需要的内部 `scoop.core.__scoop_task_create`，以及 `__scoop_task_from_result` / `__scoop_task_join` intrinsic 路径。
  - `crates/scoopc/src/llvm/codegen/runtime_symbols.rs` 与 `crates/scoopc/src/llvm/codegen/runtime_abi.rs` 已同步删掉只服务旧 executor/task surface 的符号常量与声明入口；Task 相关 runtime ABI 现只剩 `scoop_task_create`、`scoop_task_from_result`、`scoop_task_join` 三个内部 helper，对齐 `T4009a2` 后已经收缩过的 runtime allowlist。
  - `__scoop_task_create` 的 codegen 错误标签已同步改写为内部 helper 名称，避免继续把已删除的公开 `taskCreate` surface 当作当前合同的一部分。
- 已验证：
  - `cargo fmt --check`
  - `cargo test -q -p scoopc async_task_ir_uses_task_create_and_internal_join -- --nocapture`
  - `cargo test -q -p scoop_runtime --test task_spawn_join`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (357)`）
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1070)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4009a2

### T4009b [DONE] 定义 `Task.poll()` / `step()` / `Poll<T>` 合同，并隐藏 raw continuation
- 范围：
  - 明确 `Task<T>` 是 general API，`Continuation<T, eff E>` 是 advanced API；Task 内部 suspended-state / continuation carrier 改为私有实现细节。
  - 定义 `Task.poll()` / `step()` 与 `Poll<T>` 的对象模型、返回合同与 manual stepping 语义，不依赖 executor framework 才能成立。
  - `Task<T>` 作为普通 class 的内部状态要形成统一叙事：初始 lazy state、pending suspended-state carrier、completed result 三者的可见边界明确；如内部借助 continuation wrapper，也不能把它泄漏成默认 API。
- 验收：
  - `Task<T>` 可在 manual polling / stepping 下自洽成立，且 raw continuation 不再是默认 async API。
  - `SCOOP_FULL_SPEC.md` / `SCOOP_RUNTIME.md` / 相关 sysroot 文档的 `Task` / `Continuation` 边界表述一致。
- 已完成：
  - `sysroot/core.scoop` 现已定义 `Poll<T>`、`Task.poll()`、`Task.step()` 与内部 `__TaskStepResult` / `__scoop_task_step_pending` / `__scoop_task_step_ready` helper；对调用者暴露的 async surface 现在统一以 `Task<T> -> Poll<T>` 的手动 stepping 合同表达 pending/ready 状态，不再要求直接操作 raw continuation。
  - runtime `runtime/c/scoop_task.c` 已新增 `CREATED / RUNNING / PENDING / COMPLETED` 状态机与 `scoop_task_poll` 主线；`scoop_task_join` 现改为基于 poll 循环驱动，`scoop_task_step_pending` / `scoop_task_step_ready` 则统一承担 async body step 函数与 task 对象之间的私有结果传递。
  - HIR lowering / LLVM codegen 已把 `async {}` / `async fun` 收口为 task-private step function：`Async.await(...)` 在 async body 内被改写为 handled 形式，正常完成走 `__scoop_task_step_ready(...)`，挂起走 `__scoop_task_step_pending(...)`，从而把 suspended-state carrier 保持在 `Task` 私有实现边界内。
  - 收口过程中还修复了两个真实回归：一是 async body 中 synthetic `__task_ready_value` block 里的 `return` 现在会继续流向 `step_ready(...)`，不再错误终结为函数级返回；二是 unified state machine 对局部 initializer “是否消费前一个 `last_value`” 的判断现改由 planner 显式标记，修复了 `continuation_resume_continuation.scoop` 中 self-contained nested handle initializer 被吞掉的问题。
  - `SCOOP_FULL_SPEC.md` / `SCOOP_RUNTIME.md` 与新的 run-pass / runtime 回归已同步更新，`Task` / `Poll` / `Continuation` 的边界表述现已对齐。
- 已验证：
  - `cargo test -q -p scoopc async_task_resume_ir_does_not_replay_original_await_site -- --nocapture`
  - `cargo test -q -p scoopc async_task_ir_uses_task_create_and_internal_step_result_helpers -- --nocapture`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/task_poll_step_manual_basic.scoop`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/async_fun_task_runtime_basic.scoop`
  - `cargo run -q -p scoop -- run tests/fixtures/run-pass/continuation_resume_continuation.scoop`
  - `cargo fmt --check`
  - `cargo run -q -p scoop_tools -- spec-fixtures check`
  - `cargo test --all`
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1071)`）
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4009a3

### T4009c [DONE] 将 `spawn` / `join` 从当前 `Task` core 语义中移出，并明确留待后续 structured concurrency
- 范围：
  - `spawn`、`join`、调度、queueing、wakeup registration 不再作为当前 `Task` core 的定义性语义，也不再驱动 `Task<T>` 的对象模型设计。
  - 当前代码库中若存在“先执行 body 再包装 completed task”之类的临时 `spawn` 语义，应明确移除、降级或改写为 deferred note，避免继续误导 `Task` 设计。
  - `SCOOP_FULL_SPEC.md` / `TODO.md` / 相关注释对 structured concurrency 的表述需要同步，不再把当前半实现 surface 说成正在定型中的 core。
- 验收：
  - `Task<T>` core 与 `spawn` / `join` / executor scheduling 叙事彻底解耦。
  - 文档中剩余的 `spawn` 内容只以明确 deferred item 形式出现。
- 已完成：
  - parser / AST 继续保留 `spawn { ... }` 与 `join expr` 的语法壳，但 typecheck 现在会统一报 `structured_concurrency_deferred`，明确它们不属于当前 `Task` core；相关正向 `spawn/join` 夹具已替换为 `async {}` / `await` / handled `Async.await` 主线，并新增 `spawn_deferred_is_error` / `join_deferred_is_error` 负向回归。
  - HIR lowering 不再把 `spawn` / `join` 落到 `__scoop_task_from_result` / `__scoop_task_join` 的“最小可执行语义”；它们现在只保留为 deferred 占位节点。`sysroot/core.scoop`、`runtime/c/scoop_task.c`、`SCOOP_FULL_SPEC.md` 与 `SCOOP_RUNTIME.md` 也已同步改写为“structured concurrency（含公开 `spawn/join`）留待后续；当前仅保留内部 task helper”。
  - 在验证过程中额外暴露并修复了一个既有 `Task` core 裂缝：当 `async` 体的尾值直接是 `await task` 且结果为 GC 引用类型（例如 `String`）时，结果会在进入 `step_ready(...)` 前丢失。`crates/scoopc/src/hir/lower/block.rs` 现会先把 async body 的尾值物化到合成局部，再交给 `__task_ready_value` / `step_ready(...)`，从而让 `Task<String>` 的嵌套 await 与现有 `Task<Int>` 主线一致。
  - `ISSUES.md` 第 2 条已从“pollable object 尚未定型”收窄为“`Task` core 已定型，下一项只剩 stable handle 作为 wake token / reactor identity 的合同”。
- 已验证：
  - `cargo test -q -p scoopc async_task_ir_uses_task_create_and_internal_step_result_helpers -- --nocapture`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck`（`fixtures: ok (336)`）
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/run-pass`（`fixtures: ok (357)`）
  - `cargo run -q -p scoop -- test`（`fixtures: ok (1072)`）
  - `cargo run -q -p scoop_tools -- spec-fixtures check`（`spec fixtures: ok (1)`）
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 依赖：T4009b

### T4009h [TODO] 收口 stable handle 作为 wake token / reactor identity 的合同
- 范围：
  - 复用现有 stable opaque GC handle（`GcHandle.raw: UIntPtr`）作为 reactor / OS callback / future executor 的 wake token，不再以 pinned `Task` ref 作为长期注册形态。
  - 明确 handle 的 ownership、保活、drop、跨 FFI 传递与回调 round-trip 合同；task/object 在 OS event polling 期间只需要被 handle 保活，不需要保持 pinned。
  - 收口 stale token / cancelled registration / lookup failure 的语义缺口；若现有语言级 `handleGet` 与 runtime contract 尚不一致，需要把修补或 deferred follow-up 明确写入任务产出。
  - 明确 `Pinned` 只用于短时裸地址借出，不承担 long-lived identity / wake token 语义。
- 验收：
  - reactor / callback / future executor 可统一以 stable handle 识别并唤醒对象，而不是依赖 pin。
  - `SCOOP_FULL_SPEC.md` / `SCOOP_RUNTIME.md` / 相关 sysroot 文档对 handle / pin 的职责划分一致。
- 依赖：T4009c

### T4009R [TODO] Review：确认 `Task` 本体已脱离 executor 前提
- 重点：
  - `Task` 必须能在 manual polling 下成立，且作为普通 Scoop class 自洽。
  - `async {}` / `async fun` 只能体现为 `Task` sugar，不再依赖 task-specific special codegen 或 executor ABI。
  - raw continuation 不应继续成为易误用的默认 API。
  - 当前 executor implementation 已从 core/runtime 叙事中移除；executor 相关内容若仍未设计，只能作为明确的 deferred item 留下。
  - stable handle / pin 的职责分离要与 `Task` / reactor 叙事一致。
- 依赖：T4009h

## T4010：值类型不可变语义与 `with`

### T4010 [TODO] 在保持不可变 value semantics 的前提下收口 `with` 与声明人体工学（拆分执行）
- 说明：
  - 最新 `ISSUES.md` 第 7 条已经把方向收窄为“继续保持值类型不可变”，当前缺口不再是支持字段级写回式 `var`，而是 `with` 仍只覆盖 `struct`、尚未泛化到 tuple / enum 等其它值类型，以及字段默认值这类 immutable-friendly 声明便利性仍未覆盖。
  - 为避免把“值更新语义”与“声明人体工学”再次搅在一个大任务里，现拆分为 `T4010a -> T4010b -> T4010R`。
- 验收：
  - 子任务全部完成后，`ISSUES.md` 第 7 条收窄或关闭。
- 依赖：T4009R

### T4010a [TODO] 将 `with` 从 `struct` 泛化到 tuple / enum 等值类型
- 范围：
  - `with` base 不再只接受 `struct`；tuple、enum payload 等值类型也进入统一的 immutable copy-update 主线。
  - 继续复用现有多段字段路径与并行冲突检查语义，避免为 tuple / enum 单独开 ad-hoc 更新规则。
  - 新增 typecheck / run-pass / lowering regression，覆盖 nested path、并行冲突与 initializer 单次求值语义。
- 验收：
  - `with` 在 tuple / enum / struct 上都走同一条“复制后更新”主线，不回流到字段写回式可变语义。
- 依赖：T4010

### T4010b [TODO] 补齐值类型字段默认值与 immutable-friendly 声明人体工学
- 范围：
  - 为值类型声明补齐字段默认值等声明便利性，避免 immutable value object 仍需样板式全量显式初始化。
  - 保持与 `with` 的 copy-update 语义一致：默认值只影响构造 / 声明入口，不引入运行期可变写回。
  - 如规范文字、sysroot 示例或相关注释需要调整，在本任务内显式同步相应文档任务。
- 验收：
  - `ISSUES.md` 第 7 条中“字段默认值这类声明便利性仍未覆盖”的部分收窄或关闭。
- 依赖：T4010a

### T4010R [TODO] Review：确认值类型仍保持整体不可变
- 重点：
  - 不接受把 `with` 扩展成字段级写回式 `var`。
  - 不允许借“默认值人体工学”重新引入可变值类型叙事。
- 依赖：T4010b

## T4011：`when` 的无 binder or-pattern 子集

### T4011 [TODO] 先收口“无 binder 的 payload or-pattern”
- 说明：
  - `ISSUES.md` 第 8 条当前明确建议先支持 `A(..) | B(..)` / `A(_) | B(_)` 这类“只判别、不绑定”的 payload or-pattern；`A(x) | B(x)` 的 binder-sharing 以及 bare `A | C` 的“忽略 payload”语法糖都不应在这一轮一起放开。
- 范围：
  - resolver / typecheck / lowering / runtime matching 支持无 binder 的 payload or-pattern。
  - 现有 binder 声明与作用域规则保持不变：`WhenPat::Or` 仍不引入 binder，带 binder 的 or-pattern 继续报精确错误。
  - 新增 parse / typecheck / run-pass regression，覆盖 variant payload 判别、wildcard payload 与 mismatch 路径。
- 验收：
  - `ISSUES.md` 第 8 条收窄到明确的 binder-sharing 后续设计，或直接关闭。
- 依赖：T4010R

### T4011R [TODO] Review：确认 or-pattern 没有偷偷放开 binder-sharing 或 bare-variant sugar
- 重点：
  - 不允许把 `A(x) | C(x)` 之类分支宽松合成 `Any` binder。
  - 不允许把 bare `A | C` 偷偷扩成“忽略 payload”的 parser 糖。
- 依赖：T4011

## T4012：annotation model 与 built-in annotations

### T4012 [TODO] 收口 annotation class model 与 non-inline built-in annotations（拆分执行）
- 说明：
  - `ISSUES.md` 第 9 条当前同时包含两类缺口：annotation class 仍停在 data-only 主构造参数子集，built-in annotation 也仍只覆盖 `Unsafe / Safe / NoGC / Extern / Intrinsic / CallingConvention`。
  - 其中 `@Inline` 与 `ISSUES.md` 第 10 条的 legacy inline 清理强耦合，因此本组任务先只覆盖 annotation model 与 non-inline built-ins，`@Inline` 明确顺延到 `T4013` 统一收口。
- 验收：
  - 子任务全部完成后，`ISSUES.md` 第 9 条至少收窄到与 `@Inline` 交叉的剩余项，或被完全关闭。
- 依赖：T4011R

### T4012a [TODO] 让 annotation class 进入统一 nominal model
- 范围：
  - annotation class 不再只限于“主构造参数承载数据”的 data-only 子集；supertypes / implements / type body 进入统一的 parser / resolver / typecheck 主线。
  - 保留 annotation-specific 约束，但这些约束应建立在统一 nominal model 之上，而不是继续依赖单独的“annotation class 特殊 AST 子集”。
  - 补充 parse / typecheck regression，覆盖 richer annotation declaration 与非法组合诊断。
- 验收：
  - `ISSUES.md` 第 9 条中“annotation class 不支持继承 / 实现接口 / 类型体”的部分收窄或关闭。
- 依赖：T4012

### T4012b [TODO] 补齐 non-inline built-in annotations 的编译器语义
- 范围：
  - built-in annotation 的编译器识别不再只停在 `Unsafe / Safe / NoGC / Extern / Intrinsic / CallingConvention`。
  - 先收口 `@Deprecated`、`@AllowIntrinsic`、`@Suppress` 等 non-inline built-ins 的解析、诊断与行为；`@Inline` 明确留到 `T4013`，避免再次把 inline 做成控制流语义。
  - 如公开语义或诊断文本变化涉及规范 / 文档，在本任务中显式列出同步项，而不是直接跳过。
- 验收：
  - `ISSUES.md` 第 9 条中除 `@Inline` 外的 built-in annotation behavior 缺口收窄或关闭。
- 依赖：T4012a

### T4012R [TODO] Review：确认 annotation system 不再停在 data-only + 少数 hard-coded built-ins
- 重点：
  - richer annotation model 不能只是对个别 built-in annotation 额外开后门。
  - `@Inline` 的剩余交叉项必须明确移交给 `T4013`，不能在本条 review 里含混带过。
- 依赖：T4012b

## T4013：legacy `inline` / non-local return 语义清理

### T4013 [TODO] 删除 `inline` 的控制流语义残留，并把 `@Inline` 收口为非语义优化提示
- 范围：
  - 移除 inline 函数 lambda 实参中的 non-local return 语义门禁、错误文案与对应 fixture 口径，不再让 `inline` 参与控制流语义。
  - `@Inline` 若保留，则只作为优化提示存在，不引入任何额外的 non-local return / break / continue 语义。
  - 若规范文字、spec fixtures 或 sysroot 注释需要同步，在本任务内显式列出相应更新与 `spec-fixtures check` 验收。
- 验收：
  - `ISSUES.md` 第 10 条收窄或关闭。
  - `ISSUES.md` 第 9 条中与 `@Inline` 相关的剩余交叉项一并关闭。
- 依赖：T4012R

### T4013R [TODO] Review：确认 `inline` / `@Inline` 都不再参与控制流语义
- 重点：
  - 不允许把旧的 non-local return 规则换个入口继续保留。
  - 若未来还要重新引入相关能力，也只能作为显式 deferred design item 留下。
- 依赖：T4013

## T4014：FFI / ABI 边界与 stable handle / pin 职责分离

### T4014 [TODO] 收口普通 `@Extern` 的 effect-impermeable 边界与 stable handle / pin 合同（拆分执行）
- 说明：
  - 最新 `ISSUES.md` 第 11 条当前可收口为两条：普通 FFI 边界仍缺少“effect / continuation / non-local control 不可穿透”的明确契约；long-lived GC object identity 也仍需要以 stable opaque handle 而不是 pin 来跨越 ABI / reactor 边界。
  - `Pinned` 在本阶段的定位应收窄为“短时裸地址借出”；它不再承担 wake token、long-lived identity 或长期注册语义。
  - 这两条既共享 FFI 边界主题，又会分别影响 typecheck contract 与 sysroot / ABI surface，因此拆分为 `T4014a -> T4014b -> T4014R`。
- 验收：
  - 子任务全部完成后，`ISSUES.md` 第 11 条收窄或关闭。
- 依赖：T4013R

### T4014a [TODO] 明确普通 `@Extern` 不能穿透 effect / continuation / non-local control
- 范围：
  - 普通 `@Extern` ABI 的 typecheck / lowering / runtime contract 明确禁止 effect、continuation 与 non-local control 穿越边界。
  - `@NoGC`、`Ptr<T>` / `UIntPtr` / stable handle 桥接与普通 FFI 约束之间的关系要形成统一叙事，而不是继续依赖隐含约定。
  - 补充 typecheck / docs / regression，覆盖违规签名、违规调用与允许的显式桥接路径。
- 验收：
  - 普通 FFI 接口不再暴露隐藏的 GC / effect 语义；边界契约在诊断与文档上都可见。
- 依赖：T4014

### T4014b [TODO] 完善 stable handle 的 FFI / reactor 合同，并把 `Pinned` 收口为短时裸地址借出
- 范围：
  - stable handle（`GcHandle` / `raw: UIntPtr`）要形成统一的 FFI / reactor / callback round-trip 合同，作为 long-lived object identity / wake token 的标准 opaque surface。
  - handle 的创建、保活、drop、stale token / cancelled registration / lookup failure 边界要在语言文档、runtime 文档与 ABI 叙事中一致；不能再让 pin 与 handle 的职责混杂。
  - `Pinned` 继续保留为“短时把 GC 对象借成稳定裸地址”的 unsafe bridge；sysroot / runtime 文档中不得再把它描述为长期 token。
  - 补充 extern signature / round-trip regression，覆盖 handle 传递、回传、drop，以及 pin/unpin 的短时地址借出边界。
- 验收：
  - `ISSUES.md` 第 11 条中关于 stable handle / pin 职责分离的剩余描述收窄或关闭。
- 依赖：T4014a

### T4014R [TODO] Review：确认普通 FFI 边界不再隐含 GC / effect 语义
- 重点：
  - 不允许普通 `@Extern` ABI 继续默许 effect / continuation 穿越。
  - stable handle 必须成为 long-lived identity / wake token 的统一合同；`Pinned` 只能停留在短时裸地址借出语义。
  - handle / pin 的边界不能只是文档口头概念；必须与 ABI surface 和类型系统叙事对齐。
- 依赖：T4014b

## T4015：const / comptime 扩展

### T4015 [TODO] 将 const/comptime 从最小纯算术子集扩到可用的纯计算模型（拆分执行）
- 说明：
  - 最新 `ISSUES.md` 第 12 条把当前限制拆成三层：`const fun` 解析仍只支持同文件 + 名字/参数个数的最小选择；常量 evaluator / interpreter 仍只覆盖很窄的纯表达式子集；header phase 仍对 effect row / `eff` 参数采取一刀切早退。
  - 这三层依赖顺序不同，因此拆分为 `T4015a -> T4015b -> T4015c -> T4015R`。
- 验收：
  - 子任务全部完成后，`ISSUES.md` 第 12 条收窄或关闭。
- 依赖：T4014R

### T4015a [TODO] 收口 `const fun` 的解析 / 选择 / 跨文件调用主线
- 范围：
  - `const fun` 解释器不再只按“同文件 + 函数名 + 参数个数”做最小选择；需要接入统一的声明处上下文、重载与跨文件解析主线。
  - `const fun` 的 call-site 选择、generic 实例化与 declaration context 要与普通函数解析保持可解释的一致性，而不是继续依赖 comptime 私有旁路。
  - 补充对应单测 / regression，覆盖跨文件 const 调用、重载选择与错误路径。
- 验收：
  - `ISSUES.md` 第 12 条中“const fun 解释器当前只支持同文件、按函数名 + 参数个数的最小选择”的部分收窄或关闭。
- 依赖：T4015

### T4015b [TODO] 扩展纯 comptime evaluator / interpreter 到控制流、局部声明与循环等常见结构
- 范围：
  - 常量 evaluator / interpreter 从“字面量 + 一元/二元运算”扩展到更完整的纯计算子集，包括控制流、局部声明与循环等常见结构。
  - 继续保持纯计算前提，不把 effectful execution 偷偷放进 comptime；必要时通过明确 diagnostics 区分“纯但未支持”和“语义上不允许”。
  - 补充 regression，覆盖条件分支、局部绑定、循环与跨函数纯计算。
- 验收：
  - `ISSUES.md` 第 12 条中“常量 evaluator 仍只覆盖很窄纯计算子集”的部分收窄或关闭。
- 依赖：T4015a

### T4015c [TODO] 重新收口 `const fun` 的 effect-row / `eff` 参数 contract
- 范围：
  - `const fun` 对 non-`Pure` effect row 与 `eff` 参数的限制不能继续停留在“一刀切早退但没有明确 contract”；需要决定并实现可支持的纯兼容子集，或把不支持部分写成显式 deferred contract 与精确诊断。
  - typecheck、文档与 comptime interpreter 的边界表述必须一致，不再出现 header phase、解释器与 spec 三处口径分裂。
  - 若本轮仍选择保守限制，必须把 `SCOOP_FULL_SPEC.md` / 相关文档的同步任务纳入验收。
- 验收：
  - `ISSUES.md` 第 12 条中关于 non-`Pure` effect row / `eff` 参数的剩余描述收窄或关闭。
- 依赖：T4015b

### T4015R [TODO] Review：确认 const/comptime 不再只靠“同文件 + 名字/参数个数 + 字面量求值”的最小旁路
- 重点：
  - 不允许在 parser/typecheck 接受更多语法后，解释器仍偷偷回退到最小选择模型。
  - comptime 的“纯计算”边界必须由统一 contract 说明，而不是散落在多个早期 gate 里。
- 依赖：T4015c
