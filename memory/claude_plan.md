# 本轮执行计划

## 目标

本轮只完成 `TODO.md` 中第一个未完成任务；如果发现该任务过大，则先拆分任务并更新 `PLAN.md` 与 `TODO.md`，随后只执行拆分后的第一个子任务。

## 约束与执行原则

1. 先检查最新一次提交是否提到已知问题或遗留修复项；若有，优先一并处理。
2. 然后读取 `TODO.md`，定位第一个未完成任务。
3. 如任务过大，先拆分为可执行子任务，并同步更新 `PLAN.md`、`TODO.md`。
4. 只实现当前轮应处理的第一个任务或子任务，不推进后续任务。
5. 实现后必须补充或更新测试，并运行足够的验证命令。
6. 保持代码无编译告警，按要求尽量覆盖 `cargo fmt`、`cargo test`、`cargo clippy --all-targets -- -D warnings`，并根据实际改动补充更有针对性的命令。
7. 完成后更新 `TODO.md`、`PLAN.md`、本文件，并提交 Git commit，然后停止。

## 当前已知信息

- 需要遵循仓库内 `AGENTS.md`/项目说明，使用中文输出。
- 已检查最新提交：`af6819f3da554b92412319cc993a5002312b7e63`，提交信息为 `[T0151] 支持 custom iterator for-loop lowering`。
- 最新提交说明中未直接声明需要先处理的额外遗留 bug；本轮继续按 `TODO.md` 首个未完成任务推进。
- `TODO.md` 首个未完成任务为 `T0152`：`Nullable 访问补齐：safe member access 支持 ref receiver / extension property`。
- 已阅读相关实现，确认该任务范围可控，本轮无需先拆分子任务。

## 初始步骤

1. 已完成：查看最新一次提交信息，确认未在提交说明中声明需要先修复的额外遗留问题。
2. 已完成：读取 `TODO.md` 与 `PLAN.md`，确定本轮目标为 `T0152`。
3. 已完成：审阅 `typecheck/expr/member.rs`、`hir/lower/expr.rs`、`resolve/scopes.rs` 与现有 fixtures，确认当前缺口主要有两处：
   - `infer_safe_member_access_expr_type` 只支持 `Option<Struct>` 字段访问，未复用普通 member access 的解析结果；
   - `lower_safe_member_access_expr` 在 `Some(v)` 分支里总是构造 `MemberAccess`，未对 extension property 做 getter 脱糖。
4. 下一步：实现上述两处修复，并补充 typecheck / run-pass / 必要的 HIR 回归。
5. 然后运行格式化、测试、lint 与必要的定向验证。
6. 最后更新 `TODO.md`、`PLAN.md`、本文件中的进度记录，提交变更并停止。

## T0152 实施方案

1. 抽取或复用普通 `member access` 的成员类型推导逻辑，使 safe member access 在 unwrap `Option<T>` 后与普通成员访问共享解析结果。
2. 确保引用接收者（例如 class / object）上的 safe 字段访问返回 `Option<field_ty>`。
3. 在 HIR lowering 的 safe member access 分支中，对 `ExtensionValue` 走与普通 member access 一致的 getter 调用脱糖。
4. 新增 fixtures，至少覆盖：
   - `Option<Class>` / `Option<Object>` 的 safe 字段访问；
   - safe extension property；
   - `None` 分支返回 `None`。
5. 跑定向回归后，再跑格式化、全量测试与 clippy。

## 进度记录

- 已创建本计划文件，后续会在关键步骤完成后持续更新。
- 已完成上下文收集与任务判断：本轮执行 `T0152`，无需拆分任务。
## 2026-04-11 接续执行记录（第二阶段）

- 接手状态：
  - 当前任务仍是 `T0152 [TODO] Nullable 访问补齐：safe member access 支持 ref receiver / extension property`。
  - 上一阶段已经完成大部分 typecheck / lowering 改造，并新增 fixture 与回归测试。
  - 当前已知阻塞是 `safe_member_access_ref_and_extension_basic.scoop` 在 build/codegen 阶段失败，报 `member access target`，说明 safe member access 的决议信息没有在所有场景下正确传递到 lowering。

- 本阶段执行计划：
  1. 先复核当前工作树与相关文件状态，确认已有改动没有丢失。
  2. 运行并定位已有的定向单测，确认哪些 safe member access 场景没有写入 `safe_member_access_resolved` side table。
  3. 修复 `ref receiver` 与 `extension property` 在 safe member access 下的 typecheck 决议补全逻辑，确保 lowering 能拿到 `ResolvedMemberRef`。
  4. 重新运行定向单测、单独 build/run fixture，确认 `T0152` 的目标行为成立。
  5. 运行完整格式化、测试、fixture、clippy 校验，确保无 warning、无回归。
  6. 更新 `TODO.md`、`PLAN.md`、本文件，标记 `T0152` 完成。
  7. 提交一次 git commit，然后停止，不进入下一个任务。

- 约束与注意事项：
  - 不回滚当前工作树里的既有改动。
  - 手工编辑一律通过 `apply_patch` 完成。
  - 如果中途发现 `T0152` 仍需再拆分，先更新 `PLAN.md` / `TODO.md` 后再执行首个子任务。

## T0152 完成记录

- 根因定位：
  - 先前已经补齐了 safe member access 在 typecheck / lowering 间共享成员决议的主路径，但 `safe_member_access_ref_and_extension_basic.scoop` 仍在 build/codegen 阶段失败。
  - 最终定位到两个条件同时成立：
    1. resolver 对 `Option<Class>` / safe extension property 的 `receiver?.member` 不会直接写回 `member.resolved`；
    2. `check_expr_stmt` 在“表达式语句里的普通调用实参”递归检查中会跳过 `SafeMemberAccess`，导致 `main` 里的 `printOptInt(..., x?.prop)` 没有触发 `infer_safe_member_access_expr_type`，于是 AST side table 缺少 `User.score` / `doubleScore` 的补写结果。

- 最终实现：
  - 保留并完成上一阶段已做的主体改造：
    - `typecheck/expr/member.rs`：safe member access 在 unwrap `Option<T>` 后复用普通 member access 解析，并补做 ref receiver / extension property 决议；
    - `typecheck/lower.rs` + `ast::File`：新增 safe member access resolution side table；
    - `hir/lower/expr.rs` / `hir/lower/sugar.rs`：safe member access 的 `Some(v)` 分支复用普通 member access lowering，并支持 extension property getter 脱糖；
    - `hir/lower/mod.rs`：新增 build-path 回归测试，断言 side table 与 lowered HIR 都保留了解析结果。
  - 新增关键修复：
    - `typecheck/expr/stmt.rs`：在 `check_expr_stmt` 中为 `SafeMemberAccess` 增加显式推导入口，确保表达式语句中的调用实参也会触发 typecheck 补写 side table。

- 回归与验证：
  - 定向单测：
    - `cargo test -p scoopc lower_for_compilation_unit_multi_files_preserves_safe_member_access_resolution`
  - 定向 build/run：
    - `cargo run -p scoop -- build tests/fixtures/run-pass/safe_member_access_ref_and_extension_basic.scoop -o /tmp/t0152.out`
    - `/tmp/t0152.out`
  - 全量验证：
    - `cargo fmt --all`
    - `cargo test --all`
    - `cargo run -p scoop -- test`
    - `cargo clippy --workspace --all-targets --message-format short -- -D warnings`
  - 结果：以上命令均已通过；run-pass fixture 输出与 golden 一致。
