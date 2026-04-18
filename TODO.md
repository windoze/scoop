# TODO（Scoop：下一轮核心语言 / codegen 与 Task 设计）

> 生成时间：2026-04-18  
> 历史归档：`TODO-4.md` / `PLAN-4.md`  
> 顺序约束：严格按当前文件中的条目顺序推进；不得跨条目并行实现。  
> 本轮以 `ISSUES.md` 中既有九项为主线；若执行中发现新的前置 blocker，需先插入到依赖它的任务之前。

## 全局约束

- 在 effect / `Task` 之前的所有条目都属于核心语言 / codegen 主线；完成前不得启动 effect / `Task` 两项。
- 每个实现任务后必须立即做 review 任务；review 只审查生产代码与规范一致性，不以测试命名代替结论。
- 若某项实现改变公开语义，必须同步 `SCOOP_FULL_SPEC.md`；若涉及运行时合同，还要同步 `SCOOP_RUNTIME.md` 或相关 sysroot 文档。
- 本轮不设计 executor framework；所有与 executor、wakeup、queueing、work-stealing、spawn scheduling 相关内容一律留待后续。

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

### T4004 [TODO] 打通顶层 `val` 的 pattern binding（拆分执行）
- 说明：
  - 复查 spec §4.2 / Appendix B.11 后确认 destructuring 仅适用于 `val`；`var` 不支持 destructuring patterns，因此原“顶层 `val` / `var`”表述收窄为“顶层 `val`”。
  - 顶层版本还同时跨越“binder 符号安装 / 类型收集”和“once-init lowering / codegen”两条主线；其中前者在落地时又拆出了“显式整体类型注解路径”和“initializer 驱动推断 + 跨文件可见性”两块。
  - 为避免单轮同时横跨 parser / resolver / typecheck / 跨文件 value table / lowering，现细化为 `T4004a1 -> T4004a2 -> T4004b -> T4004R` 顺序推进。
- 验收：
  - 子任务全部完成后，顶层 tuple / struct / enum destructuring 可跨文件引用并稳定执行。
  - `ISSUES.md` 第 6 条收窄或关闭。
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

### T4004b [TODO] 打通顶层 `val` pattern binder 的 HIR / LLVM once-init lowering
- 范围：
  - 顶层 pattern initializer 只求值一次；各 binder 复用统一投影结果，不得把 initializer 重复展开到每个 binder。
  - 非 `const` 顶层 binder 复用 `top_level_immutable_values` 主线，保持初始化顺序、可见性与循环引用失败路径稳定。
  - 新增 lowering / run-pass 回归，覆盖 tuple / struct / enum 顶层 binder 读取。
- 验收：
  - 顶层 binder 在 `main`、其它顶层 initializer 与跨文件调用中可稳定 build/run。
- 依赖：T4004a2

### T4004R [TODO] Review：确认顶层与局部 pattern binding 复用同一套语义
- 重点：
  - 不接受“顶层单独走一套 ad-hoc lowering”。
- 依赖：T4004b

## T4005：Elvis `?:` lowering / codegen

### T4005 [TODO] 把 Elvis `?:` 从静态规则推进到可执行 lowering / codegen
- 范围：
  - HIR lowering 不再落回 `Any` fallback。
  - LLVM codegen 支持 Elvis 主路径。
  - nullable / rhs type 规则与执行语义保持一致。
- 验收：
  - 对应 fixtures 从 typecheck 扩展到 run-pass。
  - `ISSUES.md` 第 13 条收窄或关闭。
- 依赖：T4004R

### T4005R [TODO] Review：确认 Elvis 不再停留在“语法通过但不可执行”
- 重点：
  - 不允许保留 parser/typecheck 接受、lowering/codegen 拒绝的裂缝。
- 依赖：T4005

## T4006：跨文件 / 跨包编译链路

### T4006 [TODO] 收口跨文件顶层值、跨文件实例化与跨包扩展解析
- 范围：
  - 顶层值类型表不再只看当前文件。
  - 单态化 lowering 支持跨文件顶层函数实例化。
  - 扩展函数解析不再限于同包。
- 验收：
  - 新增多文件 / 多包 regression。
  - `ISSUES.md` 第 14 条收窄或关闭。
- 依赖：T4005R

### T4006R [TODO] Review：确认 compilation-unit 维度规则已统一
- 重点：
  - 不允许只靠“入口文件特权”维持通过。
- 依赖：T4006

## T4007：RTTI 参数化支持

### T4007 [TODO] 为 RTTI 补齐泛型与 `eff` 参数化类型支持
- 范围：
  - generic type 与带 `eff` 参数的类型不再直接 `unsupported_generic_type`。
  - 运行时类型描述符与前端类型表示保持一致。
- 验收：
  - 新增 RTTI 定向测试与必要的文档同步。
  - `ISSUES.md` 第 15 条收窄或关闭。
- 依赖：T4006R

### T4007R [TODO] Review：确认 RTTI 不再只覆盖未参数化类型
- 重点：
  - 不允许对泛型 / `eff` 类型继续静默跳过或降级成未参数化描述符。
- 依赖：T4007

## T4008：effect / continuation 完整性

### T4008 [TODO] 补齐 `Task` 手动 stepping 所需的 effect / continuation 语义缺口
- 范围：
  - richer effect polymorphism 与 continuation 类型语义。
  - receiver effect op 与相关 lowering。
  - escape continuation 组合能力，避免多 suspend / 多 await 仍要拆成多段 `handle` 的现状。
- 验收：
  - `ISSUES.md` 第 1 条收窄或关闭。
  - 文档明确区分“已支撑 `Task` manual stepping 的能力”和“仍留待后续的 executor 语义”。
- 依赖：T4007R

### T4008R [TODO] Review：确认 effect 完整性收口没有引入新的 shape-based lowering
- 重点：
  - continuation / effect codegen 不能回流到按源码形状补丁选路。
- 依赖：T4008

## T4009：`Task` 设计定型

### T4009 [TODO] 把 `Task<T>` 定型为通用的 pollable object，并隐藏 raw continuation
- 范围：
  - 明确 `Task<T>` 是 general API，`Continuation<T, eff E>` 是 advanced API。
  - 定义 `Task.poll()` / `step()` 与 `Poll<T>` 合同。
  - 支持 manual stepping，不依赖 executor framework 才能成立。
  - 清理 executor-centric、handle-based `Task` 叙事与对应文档。
- 验收：
  - `ISSUES.md` 第 2 条收窄或关闭。
  - `SCOOP_FULL_SPEC.md` 对 `Task` / `Continuation` / async surface 的边界表述一致。
  - 如 runtime / sysroot 合同改变，相关文档同步更新。
- 依赖：T4008R

### T4009R [TODO] Review：确认 `Task` 本体已脱离 executor 前提
- 重点：
  - `Task` 必须能在 manual polling 下成立。
  - raw continuation 不应继续成为易误用的默认 API。
  - executor 相关内容若仍未设计，只能作为明确的 deferred item 留下。
- 依赖：T4009
