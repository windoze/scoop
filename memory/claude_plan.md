## 当前执行计划（简要）

说明：按安全要求，这里记录可审计的执行计划、检查项与进度，不记录不可验证的内部推理细节。

### 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在执行前或执行中发现已有缺陷、回归、规格不匹配或实现边界缺口，则优先修复该问题，或将其作为前置任务写回 `TODO.md`/`PLAN.md` 后停止。

### 初始步骤

1. 查看最新一次 Git 提交，确认提交信息里是否提到任何既有问题、已知缺陷或待修复项。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，核对该任务上下文、依赖和当前项目阶段。
4. 如当前任务过大或依赖不清晰，则拆分任务并更新 `TODO.md` / `PLAN.md`，本轮只执行拆分后的第一项。

### 执行原则

1. 不通过缩小范围、规避路径、弱化测试或特殊分支来绕过真实问题。
2. 若测试、审查或实现中发现既有问题，立即转为优先工作。
3. 任何关键进展、计划变更、阻塞原因都同步更新本文件。

### 预期执行流程

1. 识别并确认本轮目标。
2. 实现目标或前置修复。
3. 运行相关验证：
   - 至少运行与改动直接相关的测试。
   - 如改动影响面较大，补充运行更广范围测试。
   - 结束前运行 `cargo clippy --all-targets -- -D warnings`（如时间/依赖允许且与改动相关）。
4. 更新 `TODO.md`、`PLAN.md` 与本文件。
5. 提交 Git commit，提交信息明确描述本轮完成内容。
6. 停止，不继续下一个任务。

### 进度

- [x] 已写入初始计划
- [x] 检查最新提交
- [x] 检查 `TODO.md`
- [x] 检查 `PLAN.md`
- [x] 确认本轮任务
- [x] 实现与验证
- [ ] 更新文档与提交

### 当前结论

1. 最新提交仅有标题 `[T4015a2] Support generic const fun instantiation in comptime`，提交正文没有额外声明必须先修的既有问题。
2. `TODO.md` 中第一个未完成任务是 `T4015b`。
3. 代码核对后确认：
   - `const fun` 的前端 header/typecheck 目前没有额外拒绝 `var` / `while` / 普通 `for` / 普通 `if`；
   - 当前真正的 blocker 在 const/comptime 解释器：`if` / block / `do` / assignment / `while` / 普通 `for` / `break` / `continue` 仍报 unsupported；
   - `comptime if` / `comptime for`、局部 `val`、generic `const fun` 调用等一部分能力已经完成，因此 `T4015b` 文案比剩余缺口更宽，需要在收口时同步修正文档/注释。

### 本轮实施要点

1. 扩展 const/comptime 表达式/语句执行能力，优先覆盖：
   - 普通 `if` 表达式；
   - block / `do` block；
   - 局部 `var` 与局部赋值；
   - `while` / 普通 `for`；
   - `break` / `continue`。
2. 保持纯计算边界不变：继续拒绝 effect、lambda、`when` 等尚未接通的路径，并给出结构化诊断。
3. 增加 regression：
   - Rust 单测覆盖新的控制流/局部变量/循环；
   - `tests/fixtures/comptime` 增加端到端 fixture，证明 const val 折叠与跨函数纯计算都能经过新主线。
4. 修正已发现的文档/注释不一致：
   - `crates/scoopc/src/comptime/mod.rs` 与相关模块顶部说明要同步到真实能力边界；
   - 必要时同步 `ISSUES.md` 对第 12 条的描述，使其与实际实现一致。

### 当前进度更新

1. `T4015b` 已实现完主体：
   - evaluator 已支持普通 `if`、block/`do`、assignment；
   - interpreter 已支持局部 `val/var`、assignment、`while` / 普通 `for`、`break/continue`；
   - block tail value 已修正为尊重分号语义。
2. 执行中发现并已修复一个既有问题：
   - 递归 const fun 的默认 `recursion_limit = 64` 在当前解释器栈帧大小下会先触发测试线程 stack overflow，无法稳定返回 `scoop::comptime::recursion_limit_exceeded`；
   - 已把默认门限收紧到 `48`，恢复稳定诊断。
3. regression 已补齐：
   - 新增 fixture：`tests/fixtures/comptime/const_fun_control_flow_locals_loops_basic.scoop` / `.comptime`
   - 新增 Rust 单测，覆盖普通 `if`、局部 `val/var`、assignment、`while`、普通 `for`、`break/continue`、嵌套在 call/binary 里的 `if`、const val initializer block。
4. 文档/计划已同步：
   - `TODO.md` 已把 `T4015b` 标记为完成并记录验证项；
   - `PLAN.md` / `ISSUES.md` / `crates/scoopc/src/comptime/*.rs` 注释已更新到当前能力边界。
5. 本轮验证结果已收口：
   - `cargo fmt --check`
   - `cargo test -p scoopc const_eval_ -- --nocapture`
   - `cargo run -p scoop -- test --fixtures tests/fixtures/comptime`
   - `cargo test --all`
   - `cargo clippy --all-targets -- -D warnings`
   - `cargo run -p scoop -- test`（`fixtures: ok (1204)`）
6. 当前待完成事项：
   - 仅暂存本轮相关文件，排除无关改动 `run_agent.sh`
   - 提交 Git commit，随后停止，不继续 `T4015c`
## 2026-04-25 本轮执行计划

1. 先核对上一轮已完成的 `T4015b` 改动是否齐全，并确认最新提交没有额外遗留问题需要先处理。
2. 检查工作区差异，确认仅提交本轮相关文件，避免误纳入无关改动（尤其是 `run_agent.sh`）。
3. 如有必要，补充更新本文件中的执行状态与验证结论。
4. 以 `T4015b` 为边界完成收尾：确认 `TODO.md` / `PLAN.md` / `ISSUES.md` 状态一致，并准备提交。
5. 提交本轮改动到 Git，提交信息使用任务编号，随后停止，不继续下一个任务。

## 2026-04-25 收尾进度

1. 已核对最新提交：`[T4015a2] Support generic const fun instantiation in comptime` 未声明额外必须先修的遗留问题。
2. 已核对任务状态：
   - `TODO.md` 中 `T4015b` 已标记为 `[DONE]`
   - `PLAN.md` 已把下一步更新为 `T4015c`
   - `ISSUES.md` 已收窄到剩余的 effect-row / `eff` 参数缺口
3. 已确认本轮提交边界：
   - 仅提交 comptime 解释器/测试/fixture 与计划文档更新
   - 不纳入无关改动 `run_agent.sh`
4. 下一步动作：
   - 暂存本轮文件并创建 `T4015b` 提交
   - 提交后立即停止
