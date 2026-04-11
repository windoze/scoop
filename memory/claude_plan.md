# 执行计划

说明：不记录不可审计的内部推理细节；以下内容是本次执行的完整对外计划、检查项与进度记录。

## 初始计划

1. 查看最新一次 Git 提交，确认提交信息中是否提到需要先处理的既有问题。
2. 阅读 `TODO.md` 与 `PLAN.md`，定位第一个未完成任务，并核对当前项目状态。
3. 如果首个未完成任务过大，先将其拆分为更小的可执行子任务，并同步更新 `TODO.md` 与 `PLAN.md`。
4. 实现当前应执行的首个任务。
5. 运行相关测试与必要的质量检查，至少覆盖本次改动涉及范围；若可行，补充执行更严格的检查。
6. 将任务状态回写到 `TODO.md` / `PLAN.md`，记录完成情况与后续影响。
7. 提交本次变更，提交后停止，不继续处理下一个任务。

## 进度记录

- 已创建本文件并写入初始计划。
- 已检查最新提交 `0e48ed3`：提交信息未引入额外独立遗留问题；其核心是在计划中把 single-arm escape-continuation 的非 `Unit` 多 direct site lowering 缺口拆成新的前置任务。
- 已阅读 `TODO.md` / `PLAN.md`：当前首个未完成任务为 `T2003c0b2b0c`，目标是打通 “single-arm escape-continuation + top-level multiple direct perform sites + 非 Unit handle 结果” 的 LLVM lowering。
- 下一步：定位最小复现、阅读相关 codegen/fixture，并判断该任务是否还能继续细拆。
- 已完成最小复现收缩：
  - 独立的 single-arm escape-continuation（multiple direct sites + 非 `Unit` 结果）样例可通过 LLVM build。
  - 真正失败的形状是：该 single-arm handle 作为 outer immediate-resume tail 中的 inner handle，且多次 direct perform 之间 arm 需要再次访问外层局部 `saved: Continuation<String>?`。
  - 该失败目前报 `scoop::llvm::unsupported_main_body / unknown local value`。
- 当前判断的根因：
  - `codegen_handle_expr_escape_continuation` 的 multi-perform step trampoline 只把 `Ref/String/Bool/Int` 当作可恢复 outer capture / body lift。
  - `Option<Continuation<String>>` 在 codegen 层是 pointer-like enum；single-site 时 arm 只在 outer function 执行，所以不会暴露问题；multiple-site 时 arm 可能在 step trampoline 的 intercept path 再次执行，此时 `saved` 未被恢复进 `cg.env`，从而触发 `unknown local value`。
- 接下来要做：
  1. 为 single-arm escape-continuation 的 capture/filter/state-field/restore/write-back 路径补 pointer-like enum 支持。
  2. 新增 run-pass fixture，覆盖“outer immediate-resume tail 中的 inner escape handle：multiple direct sites + 非 `Unit` 结果 + `Option<Continuation<_>>` 外层局部捕获”。
  3. 跑相关 LLVM fixture、全量测试与 clippy。
- 已完成实现：
  - 已为 single-arm escape-continuation 的 capture 存储协议新增 pointer-like enum 支持，并把 outer/body capture 的筛选、zero-init、restore、write-back 统一收口到同一套 helper。
  - 已同时修正 write barrier 对 pointer-niche enum 的判定逻辑，避免只看首个 variant 字段。
- 已新增回归：
  - `tests/fixtures/run-pass/effect_resume_nested_escape_handle_tail_multi_perform_nonunit.scoop`
  - 覆盖 outer immediate-resume tail 中的 inner single-arm escape handle：multiple direct sites、non-`Unit` 结果、pointer-like enum outer capture、以及两次 resume 后的剩余 tail 执行。
- 已完成验证：
  - `cargo test --all`
  - `cargo run -p scoop --features llvm -- test`
  - `cargo clippy --workspace --all-targets -- -D warnings`
- 本轮任务状态：`T2003c0b2b0c` 已完成。
- 下一步（下次调用再做）：`T2003c0b2b1`，把 sibling escape-continuation 扩展到 post-immediate multiple direct sites。
