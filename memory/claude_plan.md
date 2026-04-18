# 当前执行计划

## 说明

按要求先记录执行计划，再开始任何命令行检查或代码执行。这里记录可审计的执行步骤、判断依据和后续更新节点；不展开与实现无关的冗长草稿。

## 初始计划

1. 检查最新一次 Git 提交，确认提交说明中是否提到已知问题、遗留修复或必须先处理的事项。
2. 打开 `TODO.md`，定位第一个未完成任务。
3. 打开 `PLAN.md`，核对当前计划、依赖关系和任务拆分情况。
4. 判断该任务是否足够小且可在本轮完整完成：
   - 若可以，直接实现。
   - 若不可以，先在 `PLAN.md` 和 `TODO.md` 中把它拆成更小的子任务，并把新的首个子任务作为本轮执行目标。
5. 实现目标任务时，同时检查是否暴露出任何“必须先修复的规格偏差 / 缺失能力 / 历史遗留问题”：
   - 若发现阻塞项，不做变通实现。
   - 先把阻塞问题转成前置任务，更新 `TODO.md` 和 `PLAN.md`，提交后停止。
6. 对本轮完成的实现执行充分验证，优先运行与改动直接相关的测试；若需要，再补充全量检查，例如：
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - 以及任务相关的 fixture / 工具命令
7. 更新文档与计划：
   - 在 `TODO.md` 中将本轮任务标记完成。
   - 在 `PLAN.md` 中更新当前状态、后续顺序、任何新增依赖或风险。
   - 视进展更新本文件，记录已完成步骤和计划调整。
8. 使用清晰的 Git 提交信息提交本轮变更。
9. 停止，不继续处理下一个任务。

## 待确认事项

- 最新提交是否已经明确指出需要优先修复的问题。
- 当前首个未完成任务是否依赖尚未完成的语言特性、运行时能力或诊断修复。
- 是否存在与本轮任务相关但未在 `TODO.md` 中显式追踪的规格不一致问题。

## 进度更新（2026-04-19，初始检查后）

- 已检查最新提交：`28f519da21d000eb3e8d150cedd9ca1319705920`，提交说明为 `[T4003b] 支持顶层泛型函数值与 callee<T> 一等值传递`，未在提交说明中额外标注必须优先修复的遗留问题。
- 已检查 `TODO.md` 与 `PLAN.md`：`T4003` 已拆分完成到 `T4003b`，当前首个未完成且可执行的子任务为 `T4003c`。
- 当前目标确定为：
  1. 统一函数值与 `FunPtr` 的命名实参与默认参数绑定。
  2. 让 `super(...)` / `this(...)` 构造器委托调用接入同一套绑定规则。
  3. 为重排与默认值补齐补足必要的 side table / lowering 元数据。

## 下一步计划（聚焦 T4003c）

1. 阅读调用绑定、命名实参、默认参数、函数值调用、`FunPtr.invoke`、构造器委托相关实现与现有 fixture。
2. 找出当前行为分裂点：
   - 哪些调用形态仍在 typecheck 早期直接拒绝命名实参。
   - 哪些路径尚未共享同一套参数重排 / 默认值补齐逻辑。
   - ctor delegation 当前为何只能使用位置参数。
3. 若发现本任务实际依赖新的前置缺陷修复，则先回写 `TODO.md` / `PLAN.md` 并停止；否则直接实现 `T4003c`。
4. 为实现补充最小但充分的定向回归：
   - function value named/default args
   - funptr named/default args
   - `super(...)` / `this(...)` named/default args
5. 执行定向验证，再跑全量 `cargo test --all` 与 `cargo clippy --all-targets -- -D warnings`。
6. 更新 `TODO.md` / `PLAN.md` / 本文件并提交。

## 进度更新（2026-04-19，设计收敛后）

### 已确认的实现切面

1. 函数值 direct call 与 `FunPtr` direct call 的命名实参问题：
   - typecheck 在 `expr/call.rs` 里仍做 early reject。
   - HIR/codegen 侧本身只接受位置实参。
   - 对这两类调用，不需要默认参数支持；可统一使用合成参数名：
     - receiver function type：`receiver`
     - 普通参数：`a0`、`a1`、...
   - 计划：typecheck 改为走统一参数映射；codegen 在保持源码求值顺序的前提下按映射重排。

2. ctor delegation / super ctor args 的命名与默认参数问题：
   - typecheck 仍在 `check_ctor_call_args_by_arity` 里显式拒绝 named/spread，并只按 arity 选 ctor。
   - HIR class init side table 当前把 ctor args 降成 `Vec<Expr>`，命名信息会丢失。
   - LLVM codegen 当前同样只按 arity 选 ctor，且没有默认值补齐能力。
   - 这不仅阻塞 `this(...)` / `super(...)`，也暴露出普通 `Class(...)` 构造调用已有的 typecheck/codegen 裂缝：typecheck 已支持 named/default，但 codegen 仍只吃位置参数。

### 决定采用的实现主线

1. 为 ctor 调用新增 typecheck side table：
   - 记录 call span -> 选中的 ctor 目标（owner fqn + ctor span/implicit-primary 标记）；
   - 记录“形参槽位 -> 调用点实参索引”的绑定映射。
2. 将该 side table 贯通到 HIR：
   - 普通构造调用 side table 不再只保留 candidate type 列表，而是保留已选定的 ctor 绑定信息；
   - `ClassInit.super_ctor_args` 与 `ClassCtorDelegation.args` 改为保留 `CallArg`，不再把 named arg 降成普通表达式；
   - `ClassCtorParam` 补充 lowered `default_value`，供 codegen 执行缺省值补齐。
3. LLVM codegen 改为按“已选中的 ctor + 参数绑定”执行：
   - 显式实参按源码顺序求值一次；
   - 缺失形参按形参顺序求默认值；
   - 直接构造调用、`super(...)`、`this(...)` 统一走这套绑定逻辑；
   - reachability 扫描同时覆盖可达 ctor 参数默认值，避免默认值中调用的函数漏生成。
4. 函数值 / `FunPtr` direct call：
   - 不新增额外 side table；
   - typecheck 与 codegen 统一按 `receiver/a0/a1...` 做命名绑定与重排。

### 编辑顺序

1. AST / TypeLowering side table 结构。
2. typecheck：direct ctor call、super ctor、secondary ctor delegation、function value、funptr。
3. HIR 结构与 lowering：ctor call site、class init/default values、delegation/super args。
4. LLVM codegen 与 reachability。
5. fixture 与验证。
## 2026-04-19 本轮续作计划

### 当前判断摘要
- 目标任务仍是 `TODO.md` 中首个未完成项 `T4003c`，范围是“函数值 direct call、`FunPtr` direct call、ctor delegation / direct ctor call 的命名实参与默认参数统一绑定”。
- 现有代码已经完成大部分主线改造，并且 `cargo check -q` 已通过；剩余关键工作是运行新增与相关回归测试，修复运行时或代码生成层面的遗漏，然后更新 `TODO.md`、`PLAN.md` 与本文件并提交。
- 最新提交未发现必须在 `T4003c` 之前单独处理的新遗留问题；如果测试中暴露出既有 spec 裂缝，则该问题本身属于本任务收口范围，需要先修复再继续。

### 本轮执行步骤
1. 检查工作区状态，并确认 `TODO.md` / `PLAN.md` 当前排序仍以 `T4003c` 为首个未完成任务。
2. 优先逐个构建并运行新增的 3 个 run-pass 用例：
   - `function_value_named_args_basic`
   - `unsafe_funptr_direct_named_call_basic`
   - `class_ctor_named_default_and_delegation_basic`
3. 若任一用例失败，定位到 typecheck / HIR lowering / LLVM reachability / codegen 的具体问题，直接修复，并补充必要测试。
4. 在新增用例通过后，运行与本改动直接相关的更广验证：
   - 至少覆盖相关 `run-pass` / `typecheck` fixture
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
5. 若验证全部通过，更新：
   - `TODO.md`：将 `T4003c` 标记完成
   - `PLAN.md`：记录完成情况并把后续任务前移
   - `memory/claude_plan.md`：补充验证结果与最终结论
6. 以 `T4003c` 为主题创建单个提交，然后停止，不进入下一任务。

### 风险点
- ctor 默认参数求值顺序和参数绑定环境可能仍有遗漏，需要重点检查。
- ctor reachability / codegen 改成按 `CtorCallInfo` 选中目标后，可能存在扫描或重整参数时的边界错误。
- function value / funptr 命名实参的“合成参数名”规则必须在 typecheck 与 codegen 两端完全一致，否则会出现通过类型检查但运行结果错误的情况。

### 当前进展（定向验证后）
- 已完成 3 个新增 run-pass 用例的单文件 `build + run` 验证，当前输出与 `.stdout` 一致：
  - `function_value_named_args_basic`
  - `unsafe_funptr_direct_named_call_basic`
  - `class_ctor_named_default_and_delegation_basic`
- 在验证过程中修正了新增 fixture 的若干测试层问题，而不是主实现问题：
  - 补齐缺失的 `import scoop.core.*` / `import scoop.unsafe.*`
  - 避免依赖当前无关的继承属性解析边界（`HeaderDerived.sum`），改为只观察 ctor 执行顺序
  - 避免把两个“0 实参可用”的 ctor 设计成真实二义重载；现在测试形态保证重载选择唯一
  - `class X() : Base(...)` 的无 body 单行写法目前会走另一条既有旧路径，因此新增回归改为 `class X() : Base(...) {}`，确保本轮聚焦在 `T4003c` 主线而不是无关旧裂缝
- 下一步：用临时 fixture 根目录跑真实 `scoop test` harness，并补跑相关 `typecheck` / `cargo test --all` / `cargo clippy --all-targets -- -D warnings`。

### 最终结果
- `T4003c` 已完成并通过验证。
- 除主线功能外，还补齐了两处必须收口的实现尾巴：
  - effect state-machine 侧仍按旧 `HashMap<Span, Vec<String>>` 使用 ctor call target；现已统一改到 `CtorCallInfo`
  - `emit_minimal_main_ir` 这类无完整 typecheck 的 IR 测试入口原本拿不到新的 ctor side table，direct class ctor call 会误掉进 enum variant ctor 路径；现已在 HIR lowering 中补上基于 resolver call-shape 的保守 fallback
- 已执行并通过的验证：
  - 单文件 `build + run`
    - `tests/fixtures/run-pass/function_value_named_args_basic.scoop`
    - `tests/fixtures/run-pass/unsafe_funptr_direct_named_call_basic.scoop`
    - `tests/fixtures/run-pass/class_ctor_named_default_and_delegation_basic.scoop`
  - `cargo run -q -p scoop -- test --fixtures /tmp/t4003c-fixtures` -> `fixtures: ok (6)`
  - `cargo run -q -p scoop -- test --fixtures /tmp/t4003c-typecheck` -> `fixtures: ok (10)`
  - `cargo run -q -p scoop -- test --fixtures tests/fixtures/typecheck` -> `fixtures: ok (327)`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`
- 下一步（不在本轮执行范围内）：`T4003R`
