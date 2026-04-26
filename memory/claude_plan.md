# 当前执行计划

## 约束说明

- 按用户要求，本轮只处理 `TODO.md` 中第一个未完成任务，完成后立即停止。
- 在开始任何命令执行前，先记录本计划文件；后续若计划变化或关键步骤完成，会继续更新此文件。
- 这里记录的是可审计的执行计划、检查项和结论摘要，不包含不可审计的内部推理草稿。

## 执行步骤

1. 查看最近一次提交，确认提交信息是否提到需先修复的既有问题；若有，优先处理。
2. 阅读 `TODO.md`，定位第一个未完成任务。
3. 阅读 `PLAN.md`，理解当前计划与该任务上下文。
4. 结合代码与测试现状评估任务规模：
   - 若任务可在本轮完整完成，则直接实现。
   - 若任务过大，则先细化为更小子任务，并更新 `TODO.md` / `PLAN.md`，随后执行第一个子任务。
5. 实现任务时同时留意任何既有缺陷、规格不匹配、回避性实现或阻塞项：
   - 若发现，先修复；
   - 若不能当场修复，则在 `TODO.md` 中前置新增 prerequisite 任务，更新 `PLAN.md`，提交后停止。
6. 对本轮改动执行充分验证，至少包括相关测试；若改动涉及全局质量门槛，再运行格式化、测试与 `clippy`。
7. 更新文档与任务状态：
   - 在 `TODO.md` 中将本轮任务标记完成；
   - 在 `PLAN.md` 中更新当前状态；
   - 视需要补充 `README.md` 或代码注释。
8. 以清晰提交信息创建 git commit，然后停止。

## 待确认项

- 最近一次提交是否声明了需要先修复的历史问题。
- 首个未完成任务是否存在隐藏依赖或前置缺口。
- 当前工作树是否已有未提交改动，需避免覆盖用户已有修改。

## 当前进展

- 已查看最新提交 `f7070ddf21012f5307ebcbd80795a0100f4ca1bd`，提交标题为 `[T5000e1b0aR] Review direct-call request-binding eff args`，未在提交信息中声明新的需优先修复的历史问题。
- 已检查工作树，当前仅有本轮新增的 `memory/claude_plan.md` 尚未提交。
- 已定位 `TODO.md` 中首个未完成任务为 `T5000e1b0b 让 generic MIR template / dump-ir 收录 type-body generic member fun roots`。
- 已从 `TODO.md` / `PLAN.md` 确认该任务的直接目标：
  - `dump-ir` / generic MIR lowering 不能再把整段 type decl 仅记成 `Todo { kind: "type" }`；
  - materializer 需要能为 type-body generic member fun 找到真实 template root，并让 member direct-call 完成 monomorphic fixed-point；
  - 需要补充用户态回归，覆盖 type-body generic member fun + effect-row 实参路径。

## 下一步

1. 已完成：阅读 `mir/materialize.rs`、HIR/MIR lowering 入口并做最小复现。
2. 已确认根因：
   - `collect_generic_template_infos(...)` 已能从 type-body / object body 收集 generic member template 元数据；
   - 但 `materialize_for_dump(...)` 调用 `lower_hir_file_for_dump_with_facts(...)` 时只传入了 `lowered_hir.file`；
   - type-body / object member 方法被单独放在 `lowered_hir.member_funs` side table 中，没有进入 MIR lowering；
   - 因而 generic MIR file 里没有 `fixtures.monomorph.Box.forward` 这类 root，最终触发 `missing_mir_root_for_template`。
3. 计划中的修复：
   - 调整 MIR lowering 入口，使其在 dump 路径下同时 lowering `hir.file.items` 与 `hir.member_funs`；
   - 保持 type/object 顶层 `Todo` 占位不变，但额外发射 member `FunDecl` 的 MIR root；
   - 在 `dump-ir` materializer 路径复用同一入口，避免只修消费侧。
4. 补充回归：
   - MIR lowering 单测或 materialize 单测，锁定 type-body generic member root 已进入 generic MIR；
   - `dump-ir` 用户态回归，锁定 `Box.forward` 不再触发 `missing_mir_root_for_template` 且能产出带 `eff_args` 的实例。
5. 完成后运行相关测试、全量测试与 `clippy`，再更新 `TODO.md` / `PLAN.md` 并提交。

## 已完成结果

- 实现已完成：
  - `crates/scoopc/src/mir/lower.rs` 现会在 dump 路径同时 lowering `hir.file` 与 `member_funs`；
  - `crates/scoopc/src/mir/materialize.rs` 现会把 `lowered_hir.member_funs` 接入 generic MIR lowering；
  - `crates/scoopc/src/cone/pre_specialize.rs` 已同步对齐新签名。
- 新增回归：
  - `mir::lower::tests::dump_mir_emits_type_body_generic_member_fun_roots`
  - `mir::materialize::tests::materialize_for_dump_handles_type_body_generic_member_fun_roots`
- CLI 复现结论：
  - `/tmp/t5000e1b0b_member_root.scoop` 的 `dump-mir` 输出已包含 `fixtures.monomorph.Box.forward` root；
  - 同一用例的 `dump-ir` 已不再报 `missing_mir_root_for_template`，并会 materialize `fixtures.monomorph.Box.forward::<eff fixtures.monomorph.Boom>`。
- 已完成验证：
  - `cargo fmt --all`
  - `cargo test -p scoopc type_body_generic_member_fun_roots -- --nocapture`
  - `cargo test -p scoopc dump_materialization_inputs_keep_eff_args_for_member_direct_call_binding -- --nocapture`
  - `cargo run -q -p scoop -- dump-ir /tmp/t5000e1b0b_member_root.scoop`
  - `cargo run -q -p scoop -- dump-mir /tmp/t5000e1b0b_member_root.scoop`
  - `cargo test --all`
  - `cargo clippy --all-targets -- -D warnings`

## 收尾步骤

1. 已更新 `TODO.md` 与 `PLAN.md`。
2. 下一步执行 `git status` 复核改动集，然后提交 `[T5000e1b0b] ...` 并停止。
