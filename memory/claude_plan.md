## 当前执行计划

1. 以 `TODO.md` 为准，执行当前首个未完成任务 `CG-T07S0a8`：修复 `local_val_destructuring_nested_variant_mismatch_is_error` 中 nested variant destructuring runtime-error path 的 direct-arg tuple payload contract 缺少 source component。
2. 先复核 `TODO.md` 中该任务周边记录与最新提交，确认没有比 `CG-T07S0a8` 更早的未完成前置项，也没有需要先补录到 `TODO.md` 的直接相关 unfinished issue。
3. 复现当前失败，优先运行该任务直接对应的 build/test 命令；必要时补看定向单测或最小相关 fixture，确认失败发生在 front-end/MIR contract 还是 codegen/runtime transport。
4. 阅读 nested variant destructuring、runtime-error path、direct-arg tuple payload transport、source component 发布与消费相关实现，定位 authoritative contract 在哪里丢失。
5. 以最小正确改动修复 contract 缺口，禁止通过缩窄 fixture、变更表示方式、添加 task-private workaround 或 backend 猜 shape 规避问题。
6. 补充或更新最小回归验证，至少覆盖任务要求的定向 build/test；随后运行 `cargo clippy --all-targets -- -D warnings`，确保无 warning。
7. 完成后更新 `TODO.md`：将 `CG-T07S0a8` 标记为 `[DONE]` 并填写完成记录；仅当阶段级计划或依赖结构变化时才更新 `PLAN.md`。
8. 检查工作树，按要求提交本次任务涉及的全部未提交变更，提交信息使用 `[CG-T07S0a8] ...` 风格，然后停止。

## 约束提醒

- 只处理 `TODO.md` 当前顺序下的第一个未完成任务，不提前做后续任务。
- 若发现阻塞 `CG-T07S0a8` 的真实缺口，先把最小前置任务写回 `TODO.md` 正确位置，提交后停止。
- 不记录或暴露内部私有推理；这里只维护可执行计划、关键发现与进度。

## 当前进展

- 已确认 `TODO.md` 中 `CG-T07S0a7` 已完成，当前首个未完成任务为 `CG-T07S0a8`。
- 已确认最近提交为 `[CG-T07S0a7] Fix String literal direct call lowering`，与本轮任务直接衔接，暂未看到需要先插入的新前置任务。
- 当前工作树在开始时为干净状态。
- 已复现 `cargo run -p scoop -- build tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop -o /tmp/local_val_destructuring_nested_variant_mismatch_is_error` 失败，报 `refactor ABI tuple payload 'refactor_carrier_direct_args' 缺少 source component 1`。
- 已用 `dump-effect-lowered` 确认：`explode` 被发布为 `EffectStep` callable，且其唯一参数就是 tuple 类型 `pair: (MyOpt, Int)`；因此会生成 closure carrier/direct-entry 路径。
- 已定位根因：closure carrier 在转发“单个 tuple 形参且它本身就是 invoke-args tuple”时，把整个 raw tuple 当成 `source component 0` 再尝试重组 direct args，导致第二个 tuple component 丢失。
- 已完成代码修改：`crates/scoopc/src/llvm/codegen/effect_refactor/body.rs` 现在会在上述形状下直接转发原始 explicit args payload，避免破坏 authoritative tuple source layout；并新增 `llvm` 单测覆盖该回归点。
- 已完成验证：`cargo test -p scoopc effect_step_single_tuple_param_closure_carrier_preserves_tuple_args_payload`、`cargo run -p scoop -- build tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop -o /tmp/local_val_destructuring_nested_variant_mismatch_is_error`、`cargo run -p scoop -- test --fixtures tests/fixtures/run-pass/local_val_destructuring_nested_variant_mismatch_is_error.scoop`、`cargo fmt`、`cargo clippy --all-targets -- -D warnings`。
- 默认 `cargo run -p scoop -- test` 已越过 `local_val_destructuring_nested_variant_mismatch_is_error.scoop`；新的首个 blocker 是 `tests/fixtures/run-pass/member_call_devirt_final_receiver_direct_call_basic.scoop`，单 fixture build 在链接阶段报 `_Base.ping` undefined symbol，且引用来自 `__scoop_vtable__Base`。
- 已更新 `TODO.md`：将 `CG-T07S0a8` 标记为 `[DONE]`，并新增后续 prerequisite `CG-T07S0a9` 记录 `Base.ping`/vtable 链接阻塞。
