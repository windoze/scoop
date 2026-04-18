# 执行计划记录

## 说明

按要求先落盘计划，再开始任何命令执行。这里记录的是可审计的执行计划、关键判断依据和后续进度更新；不包含内部推理细节。

## 初始计划

1. 检查最新一次提交信息与变更摘要，确认是否明确提到已有问题、遗留缺陷或必须先处理的事项。
2. 读取 `TODO.md`、`PLAN.md`、必要时再看 `README.md`，识别第一个未完成任务及其上下文。
3. 判断该任务是否可在一次迭代中完整交付：
   - 若可完成，直接实施。
   - 若过大或被前置缺陷阻塞，先在 `TODO.md` / `PLAN.md` 中拆分、重排并记录依赖，然后只执行新的第一个子任务。
4. 实施过程中如果发现与规范不一致、已有 bug、缺失语言特性或依赖缺口：
   - 不做绕过方案；
   - 先把该问题转化为前置任务写回 `TODO.md`；
   - 在 `PLAN.md` 与本文件中说明阻塞原因和调整后的顺序；
   - 如当轮只能完成重排与记录，则提交后停止。
5. 对当前目标任务做完整实现与验证，至少覆盖：
   - 相关单元/集成/fixture 测试；
   - `cargo fmt`；
   - `cargo clippy --all-targets -- -D warnings`；
   - 必要的目标化测试命令。
6. 完成后更新文档状态：
   - 在 `TODO.md` 标记该任务完成；
   - 在 `PLAN.md` 反映当前状态；
   - 在本文件补充实际执行结果与验证记录。
7. 生成一次 git 提交，提交信息对应当前完成的任务，然后停止，不推进下一项。

## 风险与执行原则

- 若工作区存在非本次任务相关改动，不回退用户已有修改。
- 若最新提交只“提到”问题但未给出足够上下文，需要结合代码与测试确认该问题是否真实存在且未修复。
- 若任务涉及规范符合性，优先以现有测试、规范文档和实现一致性为准，不接受临时兼容性补丁。

## 进度更新

### 2026-04-19 当前判断

1. 已检查最新提交：
   - 最新提交为 `eaff1d7 [T4002R] 统一 receiver lambda 的 lowering 决议`。
   - 提交说明未额外提到新的既有遗留问题；因此没有出现“必须先补该提交中注明的 pre-existing issue”这一分支。
2. 已读取 `TODO.md` / `PLAN.md` / `ISSUES.md`：
   - 当前第一个未完成任务是 `T4003`。
   - `ISSUES.md` 第 4 条明确包含四个缺口：函数值命名实参、`FunPtr` 命名实参与 receiver function signature、`callee<T>` 一等值、constructor delegation 非位置参数。
3. 已评估 `T4003` 复杂度：
   - 该任务同时跨越 typecheck 公共调用绑定、顶层函数值表示/可执行 lowering、以及 class ctor side table/default-arg 语义，单轮完整交付风险过高。
   - 按要求需要先在 `TODO.md` / `PLAN.md` 中拆分为可管理子任务，再执行新的第一个子任务。

### 拟执行的拆分方向

计划将 `T4003` 拆为至少三个子任务：

1. `T4003a`：先打通 `FunPtr<F>` 的 receiver function type 调用语义。
   - 目标：移除 `FunPtr<F>` 对 receiver signature 的早期门禁；
   - 打通 direct call 与 `FunPtr.invoke(...)` 在 receiver 签名下的 typecheck / codegen；
   - 用独立 run-pass 回归覆盖。
2. `T4003b`：再处理顶层泛型函数值与 `callee<T>` 一等值传递。
   - 该项需要单独评估 HIR / monomorph / LLVM 表示，不与 `T4003a` 混做。
3. `T4003c`：最后收口函数值 / funptr / ctor delegation 的命名实参与默认参数映射。
   - 该项依赖额外的签名元数据或 ctor side table 扩展，后置处理。

### 当前轮具体执行计划

1. 修改 `TODO.md` / `PLAN.md`，把 `T4003` 拆成子任务，并把 `T4003a` 放为新的当前任务。
2. 实现 `T4003a`：
   - 修改 `typecheck/lower.rs`，允许 `FunPtr<F>` 的 `F` 为 receiver function type；
   - 修改 `typecheck/expr/call.rs`，让 funptr 直接调用按“receiver 作为第 0 个实参”检查；
   - 修改 `llvm/codegen/mod.rs`，让 indirect funptr call 支持 receiver 参数；
   - 必要时补充 `sysroot/unsafe.scoop` 的 `invoke` overload。
3. 新增/更新回归 fixture。
4. 运行格式化、定向 fixture、`cargo test --all`、`cargo clippy --all-targets -- -D warnings`。
5. 更新 `TODO.md` / `PLAN.md` / 本文件，提交 `[T4003a] ...`，然后停止。

### 2026-04-19 执行结果

1. 已完成任务拆分：
   - `TODO.md` / `PLAN.md` 已把原 `T4003` 拆为 `T4003a -> T4003b -> T4003c`。
   - 本轮实际执行的首个子任务为 `T4003a`。
2. 已完成实现：
   - `crates/scoopc/src/typecheck/lower.rs`：移除 `FunPtr<F>` 对 receiver function type 的 early gate。
   - `crates/scoopc/src/typecheck/expr/call.rs`：funptr direct call 改为与函数值调用一致，按“receiver 作为第 0 个显式实参”检查。
   - `crates/scoopc/src/llvm/codegen/mod.rs`：indirect funptr call 支持 receiver 参数位；`scoop.unsafe.invoke(...)` intrinsic 入口会把 named args 依 `receiver` / `a0` / `a1` 约定重排为位置实参。
   - `sysroot/unsafe.scoop`：新增 receiver 形态的 `FunPtr.invoke` overload。
   - `tests/fixtures/run-pass/unsafe_funptr_receiver_call_basic.*`：新增回归，覆盖 direct call、`.invoke(...)` 与命名实参路径。
   - `SCOOP_FULL_SPEC.md`：补充 `FunPtr<F>` 在 receiver function type 下“receiver 作为第 0 个显式实参”的说明。
3. 已完成验证：
   - `cargo fmt`
   - `cargo run -p scoop -- test --fixtures target/t4003a-fixtures/run-pass` → `fixtures: ok (3)`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/typecheck` → `fixtures: ok (326)`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
4. 后续状态：
   - `T4003a` 可标记为完成。
   - 下一项应推进到 `T4003b`（顶层泛型函数值与 `callee<T>` 一等值传递）。
