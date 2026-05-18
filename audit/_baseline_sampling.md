# UnsupportedMainBody U0 Baseline Sampling

## 统计口径

- 源码范围：`crates/scoopc/src/llvm/codegen/**/*.rs`。
- constructor 计数：匹配 `UnsupportedMainBody\s*\{`；当前所有命中同时也是 `LlvmEmitError::UnsupportedMainBody {`。
- 不排除 `#[cfg(test)]` 之后内容；本轮命中路径均在 production codegen 源文件内，后续 U1 脚本如需更精细过滤必须在 schema 中说明。
- helper/shared path 计入总数，并在 `audit/_baseline_files.txt` 中以 `route_guess=Helper` 标记。
- `kind:` 标签去重只统计字面量字段 `kind: "..."`。本轮共有 1,247 个字面量字段命中；其余 37 个 constructor 使用动态或转发式 `kind`，仍计入 1,284 个 constructor 总数。

## 复算结果

| 指标 | 当前值 | 说明 |
|---|---:|---|
| `LlvmEmitError::UnsupportedMainBody {` constructor 总数 | 1,284 | codegen 范围内与裸 `UnsupportedMainBody {` 计数一致 |
| 命中文件数 | 61 | 见 `audit/_baseline_files.txt` |
| `kind:` 字面量字段出现数 | 1,247 | 不含动态/转发式 `kind` |
| `kind:` 字面量去重数 | 982 | U1 需决定动态 kind 的 inventory 表达 |
| 单次出现 `kind:` 字面量数 | 836 | 字面量字段口径 |
| `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` entry 数 | 41 | `pipeline_user_visible_failure_policy.rs` |
| `STALE_UNSUPPORTED_MAIN_BODY_COUNTS` 总数 | 638 | 与当前 `assert_eq!(total, 638)` 一致 |
| `CODEGEN_GAP_INVENTORY` entry 数 | 21 | `codegen_gap_inventory.rs` |

## 漂移处理结论

- `PLAN.md` 与 `UnsupportedMainBody_FIX.md` 原基线为 1,277 constructors、60 files、964 unique literal kinds、825 singleton literal kinds、637 stale count。
- 当前实测已漂移为 1,284 constructors、61 files、982 unique literal kinds、836 singleton literal kinds、638 stale count。
- 本任务已同步更新 `PLAN.md` 与 `UnsupportedMainBody_FIX.md` 的阶段级基线数字；后续 U1 以本文件和 `audit/_baseline_files.txt` 为输入事实。

## Route 粗分组

| route_guess | constructor 数 | 说明 |
|---|---:|---|
| RawMirLlvm | 628 | `main/**`、`mir_body/**`、HIR stmt/expr/control/class/object 等 raw route |
| EffectLoweredLlvm | 170 | `effect_lowered/**` |
| Helper | 486 | layout/type/GC/intrinsic/call/closure/effect outcome 等 shared helper |
| Both | 0 | U0 未单独使用；shared route 先归入 `Helper`，U1 可按调用图细分 |

## 10 个抽样 entry

| 一级类 | file:line | kind | 候选 bucket | root cause hypothesis | 预期治理类 | 相关 spec |
|---|---|---|---|---|---|---|
| A | `crates/scoopc/src/llvm/codegen/gc.rs:183` | `builder has no insert block` | B-01 | GC helper 假定 LLVM builder 已定位到可插入 basic block；缺失时是 codegen 内部状态不变量破坏。 | InternalBugSentinel | `N/A:helper-invariant` |
| A | `crates/scoopc/src/llvm/codegen/call/lowering.rs:594` | `builder has no current function` | B-01 | call lowering helper 需要从当前 insert block 回溯 parent function；缺失时应统一为 helper invariant。 | InternalBugSentinel | `N/A:helper-invariant` |
| B | `crates/scoopc/src/llvm/codegen/mir_body/types.rs:75` | `pass MIR local type` | B-02 | pass MIR local 在 codegen 前必须发布完整 source type 与 codegen type；缺失说明 MIR/materialize contract 漂移。 | InternalBugSentinel | `docs/spec/language_spec-part3.md` §17 |
| B | `crates/scoopc/src/llvm/codegen/stmt.rs:72` | `assignment to immutable local` | B-08 | 对不可变 local 赋值应由 typecheck 以前端诊断拒绝，不能到 LLVM codegen 才兜底。 | FrontendReject | `docs/spec/language_spec-part3.md` §2 |
| B | `crates/scoopc/src/llvm/codegen/class_ctor.rs:45` | `class ctor call candidate class` | B-20 | ctor candidate class 与 selected/ordered args 应由 resolve/typecheck 固化；codegen 不应重新猜目标 class。 | InternalBugSentinel | `docs/spec/language_spec-part2.md` §4.2; `docs/spec/language_spec-part3.md` §4.2 |
| C | `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:750` | `effect-typed closure surface function type` | B-10 | effect-typed closure surface 需要真实 adapter/layout 支持，不能只依赖前置拒绝。 | RealImpl | `docs/spec/language_spec-part3.md` §6.1; `docs/spec/language_spec-part4.md` §12.1 |
| C | `crates/scoopc/src/llvm/codegen/effect_lowered/value.rs:1145` | `effect-typed plain adapter carrier param` | B-10 | plain-to-effect adapter 已进入合法 lowering 路径，carrier 参数 ABI 缺失应通过 adapter 实现补全。 | RealImpl | `docs/spec/language_spec-part4.md` §12.3 |
| C | `crates/scoopc/src/llvm/codegen/mir_body/transport.rs:73` | `MIR transport to String requires ordinary ToString lowering` | B-13 | composite/value transport 到 String 需要普通 ToString lowering 或明确 transport contract，而非 codegen 兜底。 | RealImpl | `docs/spec/language_spec-part1.md` §6.5; `docs/spec/language_spec-part3.md` §15 |
| D | `crates/scoopc/src/llvm/codegen/expr.rs:77` | `expression` | B-36 | `ExprKind::Missing/Todo` 到达 codegen 表示存在尚未定义或未拒绝的语法 surface；P3/P5 应给出 `INTENTIONALLY-EMPTY` 或 frontend reject 立场。 | FrontendReject | `docs/spec/language_spec-part4.md` §10; `docs/spec/language_spec-part4.md` §11 |
| D | `crates/scoopc/src/llvm/codegen/stmt.rs:247` | `statement` | B-36 | `StmtKind::Todo` 到达 codegen 表示 parser/HIR 中的占位语句未被前端消解或拒绝；应归入 spec-pending surface 并写 negative fixture。 | FrontendReject | `docs/spec/language_spec-part4.md` §10; `docs/spec/language_spec-part4.md` §11 |

## codegen gap inventory 对账

| gap_id | primary bucket | notes |
|---|---|---|
| `PIPELINE_GAPS §2.3` | B-36 | `pass MIR Todo`；第二候选 B-11 |
| `PIPELINE_GAPS §3.1` | B-10 | raw MIR effect-control terminator route；第二候选 B-05 |
| `PIPELINE_GAPS §3.2` | B-10 | raw MIR `Perform` route |
| `PIPELINE_GAPS §3.3` | B-10 | raw MIR `PerformResult` route |
| `PIPELINE_GAPS §3.5` | B-14 | runtime cast/typecheck metadata support surface |
| `PIPELINE_GAPS §3.6` | B-05 | dispatch/resume handoff contract；第二候选 B-10 |
| `PIPELINE_GAPS §3.8` | B-15 | when-pattern runtime type test gate；第二候选 B-14 |
| `PIPELINE_GAPS §3.9` | B-20 | typed class ctor selected/ordered args contract |
| `PIPELINE_GAPS §3.10` | B-03 | typed default-arg ordered call contract；第二候选 B-04 |
| `PIPELINE_GAPS §3.11` | B-12 | closure env composite transport；第二候选 B-13 |
| `PIPELINE_GAPS §3.12` | B-10 | effect-typed callable adapter regression guard |
| `PIPELINE_GAPS §3.13` | B-08 | StoreMember continuation route；第二候选 B-23 |
| `PIPELINE_GAPS §4.1` | B-13 | composite value erasure descriptor-backed boxing |
| `PIPELINE_GAPS §4.3` | B-22 | oversized enum payload boxing；第二候选 B-13 |
| `PIPELINE_GAPS §4.4` | B-22 | boxed enum payload nested payloads；第二候选 B-13 |
| `PIPELINE_GAPS §4.5` | B-13 | array composite element transport metadata |
| `PIPELINE_GAPS §5.1` | B-10 | actual outward effect set decides callable ABI；第二候选 B-03 |
| `PIPELINE_GAPS §5.3` | B-34 | cleanup-unwind contract；第二候选 B-10 |
| `PIPELINE_GAPS §5.4` | B-11 | outward-empty plain routing；第二候选 B-10 |
| `PIPELINE_GAPS §7.2` | B-14 | function-type runtime cast frontend gate；第二候选 B-30 |
| `PIPELINE_GAPS §7.6` | B-29 | GC intrinsic support-surface gate |

结论：21 个既有 gap entry 均可映射到 B-01 到 B-36。存在第二候选的 entry 已在 `notes` 中标出，U1/U2 写正式 inventory 和 bucket md 时应保留这些边界说明。

## Fixture runner 能力

- `crates/scoop/src/fixtures/expectations.rs` 当前只解析 `EXPECT`、`EXPECT-ERROR`、`ARGS`、`ENV`、run/build/cone 等头部指令。
- Rust fixture runner 源码中未找到 `IGNORE-UNTIL-FIX` 或 `ignore-until-fix` 支持。
- 因此 U5-T01 必须先扩展 test infrastructure，保证 `tests/fixtures/umb_fix/**` 中被标注的 fixture 可被自动 skip 或 xfail，而不是作为普通 failing fixture 留在仓库内。
