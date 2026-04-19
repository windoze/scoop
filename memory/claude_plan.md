## 本轮执行计划

### 约束说明
- 按用户要求，本文件会在执行命令前先写入当前的执行计划、关键假设和后续更新。
- 这里记录的是可审计的执行思路摘要与步骤，不直接转储冗长的内部推理。
- 本轮目标是：先检查最新提交是否提到已有问题并修复；再读取 `TODO.md`，定位第一个未完成任务；如任务过大则拆分并更新 `PLAN.md`/`TODO.md`；随后只完成一个任务，测试、更新文档并提交 commit，然后停止。

### 初始步骤
1. 检查最新一次 git 提交信息，确认是否明确提到遗留问题、已知缺陷或待修复事项。
2. 读取 `TODO.md`、`PLAN.md`、必要时读取 `README.md` 与相关规范文件，识别第一个未完成任务及其上下文。
3. 判断该任务是否足够小且可在本轮内完整完成：
   - 若可完成：直接实现。
   - 若过大：先拆分为更小子任务，更新 `PLAN.md` 与 `TODO.md`，然后执行新的首个子任务。

### 实施步骤
4. 阅读相关代码与测试，定位需要修改的模块。
5. 实现任务，避免引入临时性绕过方案；若发现规格不匹配或缺失前置能力，则先把该问题作为更靠前任务写入 `TODO.md`，更新 `PLAN.md` 后提交并停止。
6. 运行与本任务直接相关的测试；若改动影响范围较大，补充运行更广的测试或检查。
7. 运行质量检查，至少覆盖：
   - `cargo fmt --check`（必要时先 `cargo fmt`）
   - `cargo clippy --all-targets -- -D warnings`
   - 与任务相关的 `cargo test ...`

### 收尾步骤
8. 更新 `TODO.md`，将本轮完成的任务标记为完成；更新 `PLAN.md` 反映当前状态和后续依赖。
9. 复查工作区，确认只包含本轮相关改动且无意外回退用户修改。
10. 提交 git commit，消息使用任务编号或清晰描述。
11. 停止，不继续处理下一个任务。

### 预期风险
- 最新提交若提到已有问题，可能需要优先修复并改变本轮实际目标。
- 首个任务可能依赖尚未实现的语言特性或运行时行为；若存在此类阻塞，必须先更新任务顺序和计划，不能以规避方式继续。
- 工作区可能已有未提交改动，执行前需要识别并避免覆盖非本轮修改。

### 进度
- 已完成：初始化本计划文件。
- 已完成：检查工作区状态、最新提交、`TODO.md`、`PLAN.md`、`ISSUES.md`。
- 已完成：确认首个未完成任务是 `T4006`。
- 已完成：核对最新提交 `[T4005SR] 收口 callable-value review 遗漏的 tuple expected-context`，未发现“提交消息明确留下但尚未修复”的额外遗留问题。
- 已完成：通过现有与临时最小探针验证 `T4006` 三项主线的现状：
  - 跨文件顶层值：`run_pass_cone/top_level_val_pattern_multi_file_basic` 定向夹具通过，说明跨文件顶层值读取主线已可执行。
  - 跨文件泛型实例化：临时 cone probe 中，位于非入口文件的泛型顶层函数 `id<T>` 可被其它文件调用并成功 build/run，说明 build/run compilation-unit 主线已支持跨文件实例化。
  - 跨包扩展解析：`resolve_cone/extension_imports` 在定向 root 下通过，说明跨包 extension import / star import 解析主线已存在。
- 已完成：把上述已存在能力固化为仓库内正式 regression，并同步更新 `ISSUES.md`、`PLAN.md`、`TODO.md`。
- 已完成：新增正式 regression：
  - `tests/fixtures/run_pass_cone/cross_file_generic_top_level_val_basic`
  - `tests/fixtures/typecheck_cone/cross_cone_extension_imports`
- 已完成：定向回归、`cargo test --all` 与 `cargo clippy --all-targets -- -D warnings` 均通过。
- 已发现：全量 `cargo run -p scoop -- test` 仍被既有 run-pass `delegated_property_lazy_thread_safety_none_single_thread_ok.scoop` 阻断，错误为 `scoop::llvm::unsupported_main_body: sysroot print/println arg type`。
- 已完成：按照阻塞规则，把该问题登记为新任务 `T4006S`，插入到 `T4006R` 之前；本轮仍只完成 `T4006`，下一轮应先处理 `T4006S`。
- 进行中：整理最终工作区、准备提交。

### 计划修正
- 原先预期 `T4006` 可能需要补实现或拆分子任务；实际核查后发现，代码主线能力已大体具备，当前缺口主要是：
  - 永久 regression 覆盖不足，尤其是“非入口文件中的跨文件泛型实例化”和“跨包 extension 在 typecheck 层面的正向用例”。
  - `ISSUES.md` / 注释中仍保留过时描述，需要同步收口，避免后续继续把已完成能力当成未实现问题。
- 因此本轮执行方式调整为：
  1. 新增正式 regression：一个 `run_pass_cone` 用例覆盖“跨文件顶层值 + 跨文件泛型实例化”；一个 `typecheck_cone` 用例覆盖“跨包 extension import 后的 typecheck 主线”。
  2. 更新过时注释与 `ISSUES.md` 表述，使 issue 14 进入“已收口”状态。
  3. 运行定向测试与全量质量检查。
  4. 更新 `TODO.md` / `PLAN.md` 并提交，仅完成 `T4006` 后停止。

### 后续说明
- 虽然 `cargo run -p scoop -- test` 仍未全绿，但失败点已确认是与 `T4006` 无直接交叉的既有 delegated property codegen 裂缝。
- 为遵守“发现 blocker 就先入 TODO 再继续”的约束，已将其登记为 `T4006S`；因此本轮不会越过它去做 `T4006R` 或 `T4007`。
