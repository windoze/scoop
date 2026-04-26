# 当前执行计划

说明：按要求记录可审阅的推理摘要与执行计划；这里提供高层判断、风险点与步骤，不包含内部私有思维细节。

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果在执行前或执行中发现更早的前置问题、最新提交提到的遗留问题、或任何现存缺陷/规约不匹配，则优先修复该问题，或将其作为前置任务插入 `TODO.md` 并停止。

## 初始步骤

1. 检查最新一次 git 提交信息，确认是否明确提到待修复的既有问题。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解现有计划与任务上下文。
4. 结合任务相关代码、测试、规范与最近提交，判断该任务是否可在本轮完整完成。
5. 若任务过大，则先把任务拆分为更小的子任务，更新 `PLAN.md` 和 `TODO.md`，然后只执行新的第一个子任务。

## 执行原则

1. 不接受变通方案、夹具特判或规约偏离。
2. 发现现存 bug、回归、缺失实现边界、错误诊断、运行时问题、测试只靠绕过才能通过等情况时，立即视为当前范围内问题。
3. 若问题阻塞当前任务，则先修复；若本轮无法直接修复，则在 `TODO.md` 中把修复任务插到依赖它的任务之前，更新 `PLAN.md` 后提交并停止。

## 实施步骤

1. 收集上下文：最新提交、`TODO.md`、`PLAN.md`、相关源码/测试/规范。
2. 明确本轮目标任务及其验收标准。
3. 修改实现。
4. 运行相关测试；如有必要，逐步扩大到更完整的校验，包括格式化、测试、`clippy` 等。
5. 修复测试或实现中的所有发现问题。
6. 更新 `TODO.md` 与 `PLAN.md`，记录完成情况或依赖调整。
7. 提交 git commit，提交信息与任务编号/内容一致。
8. 停止，等待下一轮调用。

## 当前已知风险

1. 任务可能依赖尚未实现的语言特性、运行时能力或标准库行为。
2. 仓库可能存在与当前任务无关但在探测中暴露的既有问题；若属于现存缺陷且影响正确性，需要先处理或排入前置。
3. 工作区可能不是干净状态；需要避免覆盖用户已有修改。

## 进度记录

- 已创建本计划文件。
- 已检查最新提交、`TODO.md` 与 `PLAN.md`，确认最新提交未明确留下需先修复的新遗留问题；当前首个未完成任务是 `T5000e3R Review：确认 monomorphization 与 program-boundary / sysroot 收口已形成稳定前置边界`。
- 已收集到的关键证据：
  - `InstanceKey` 定义在 `crates/scoopc/src/mir/materialize.rs`，身份由 `TemplateKey + type_args + eff_args` 构成，未把 backend 符号名编码进 key 本身；
  - `llvm/frontend.rs` 的单文件 codegen 主路径已经经由 `lower_for_compilation_unit_multi_files_via_mir_instance_collection(...)`，不再走 dump-only lowering；
  - `hir/lower/mod.rs` 的 `ExplicitMirInstances` 分支仅消费 `InstanceKey` 集合来生成 HIR 兼容输出，实例发现职责已转交 MIR；
  - `mir/materialize.rs` 已存在 request-root 过滤、concrete-only request 过滤、去重队列与 per-instance materialization cache。
- 新发现的风险点：
  - `seed_requests(...)` 目前会直接把整个编译单元收集到的 `monomorph_keys` 作为初始请求种子；
  - 而单文件 frontend 会为 support sources 一并运行 `check_file_exprs_with_monomorph_keys(...)`；
  - 如果 support source 既参与 lowering，又被错误当作 request root，则其中未被入口触达的 concrete generic 调用会被错误 materialize，破坏 request-root 裁剪并增加 `-O0` / debug build 固定成本。
- 已验证并修复的既有问题：
  - 通过新增回归测试确认：当“参与 lowering 的文件集合”大于“允许贡献实例请求的 request roots”时，现有主路径缺少显式分离接口，容易把 support source 一并当作 request roots；
  - 已在 `crates/scoopc/src/hir/lower/mod.rs` 新增 `lower_for_compilation_unit_multi_files_via_mir_instance_collection_with_request_sources(...)`，显式区分 lowering 输入和 request roots；
  - 已在 `crates/scoopc/src/llvm/frontend.rs` 收紧单文件 frontend：仅入口源文件收集 `monomorph_keys`，support sources 继续参与 lowering/codegen，但不再作为实例请求根；
  - 新增/更新回归测试后，问题已被锁定且修复。
- 当前验证进度：
  - `cargo fmt --all` 已通过；
  - `cargo test -p scoopc typechecked_compilation_unit_materialization_skips_unreachable_generic_requests_from_non_request_sources -- --nocapture` 已通过；
  - `cargo test -p scoopc frontend_codegen_consumes_materialized_generic_direct_call_instances -- --nocapture` 已通过；
  - `cargo test -p scoopc single_file_frontend_keeps_distinct_effect_row_generic_instances -- --nocapture` 已通过。
- 已完成的整体验证：
  - `cargo test --all` 已通过；
  - `cargo clippy --all-targets -- -D warnings` 已通过。
- 文档状态：
  - `TODO.md` 已将 `T5000e3R` 标记为完成，并记录本轮 review 结论与修复的前置缺口；
  - `PLAN.md` 已补充 `T5000e3R` 完成记录，并把下一条待执行任务切换为 `T5000f`。
- 本轮最终结论：
  - `InstanceKey` / monomorphization 主语义、program-boundary / sysroot 最小契约、以及 request-root / per-instance cache 成本边界已形成稳定前置；
  - review 中暴露的 single-file frontend support-source request-root 泄漏已修复；
  - 下一步只剩提交本轮改动，然后停止，等待下一轮执行 `T5000f`。
