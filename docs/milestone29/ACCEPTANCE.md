# M29 验收记录

2026-10-06：M29 已完成，三平台全部适用 fixture 已覆盖。
本记录对应当日的 companion / 普通 codec 协议修订；早期实例编码和条件
conformance 的结果保留在 [PROGRESS.md](PROGRESS.md)，不计入本次正式验收。
功能与完成门见 [DESIGN.md](DESIGN.md)。

## 实现与覆盖

- `Encodable<T>.encode(value, encoder)` 和 `Decodable<T>.decode(decoder)` 都由真实
  companion / codec receiver 分派。数据继承不传递 codec 能力；JSON 两个入口均显式接收 codec。
- generic companion 按完整宿主 application 拥有独立类型、singleton 与初始化状态；
  同一 application 跨 Cone 沿普通 ODR 合并，不同实参的初始化成功或失败互不混并。
- 用户 annotation、SerialName / Transient 和字段、variant、property、构造关系通过
  共有 HIR 的静态 shape 查询与 `.slib` 保存；没有新增运行期字段或 annotation 反射表。
- struct、enum 和设计范围内 final class 支持两个方向分别 opt-in、分别补齐方法。
  合法手写、继承或 default 实现优先，字段 codec 在定义处绑定，访问和构造遵守普通语言规则。
- scalar companion、UnitEncoder / UnitDecoder、显式容器 helper 和 tuple 闭包完成组合。
  泛型数据无需编码 bound；撤销旧实例编码、容器条件 conformance 和 tuple 专用编码 IR。
- fixture 覆盖独立与组合行为、完整位置和消息的负例、各 stage golden、跨 Cone 泛型 / ODR、
  artifact-only link / run、父子 codec 独立、求值与初始化顺序、异常、default、Context 和 moving GC。

当前 fixture 声明共 2,496 项，其中 176 项属于 M29。已将三平台的通过集合与当前声明
逐项比较，全部适用用例都有通过结果，不适用集合也与目标声明一致。

## 完整文件 fixture 覆盖

每个目标使用固定的 `scoop`、`scoopc`、`scoop-link`。Darwin 的普通 `--all` 完整通过，
没有使用筛选或更新快照。Linux 的完整覆盖来自更新轮与定向复验：功能、完整诊断、链接和
运行断言照常执行，快照使用 `--update-snapshots` 后通过共有输出比较及差异审阅确认。
Linux 下表按各用例最终成功的执行结果去重，快照列包括更新和比对。

| 目标 | 通过 | 不适用 | 变体 | 执行进程 | 快照处理 | M29 通过 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `aarch64-apple-darwin` | 2,490 | 6 | 2,580 | 12,438 | 12,418 | 176 |
| `x86_64-unknown-linux-gnu` | 2,471 | 25 | 2,558 | 12,218 | 12,186 | 176 |
| `x86_64-unknown-linux-musl` | 2,473 | 23 | 2,560 | 12,224 | 12,186 | 176 |

Linux 首批通过 glibc 418 项、musl 415 项；余下更新轮分别通过 2,050、2,055 项，
各有三个旧产物摘要诊断失败。两个 generic ODR 用例及 artifact-graph 同步摘要后均完整
复验通过，补齐三项；原始失败和中断报告保留。三平台使用同一份最终编译器实现，期间只
更新预期和验证记录，没有在 Linux 更新完成后继续改动编译器代码。

Darwin 普通完整报告为 `/tmp/scoop-m29-saved-reports/m29-generic-primitive-darwin-all.json`。
Linux 同目录的来源报告使用 `m29-generic-primitive-{gnu,musl}` 前缀，后缀为
`-update-first.json`、`-update-2.json`、`-graph-update.json`，以及 GNU 的
`-odr-update.json`、musl 的 `-odr-update-2.json`。去重与覆盖汇总为
`/tmp/scoop-m29-revised-final-{gnu,musl}-coverage.json`；Darwin 集合核对为
`/tmp/scoop-m29-revised-final-darwin-verification.json`。

两 Linux 目录同步至 `5a23c18ca` 后曾再次启动普通 `--all`。按用户要求提高测试效率，
停止这两轮重复运行：停止前分别通过 485、487 项，没有失败，各余 1,986 项中断。
这两轮不计作完整普通 `--all`，也未用于增加上表覆盖。原报告单独归档为
`m29-generic-primitive-{gnu,musl}-redundant-all-interrupted.json`，日志为
`/tmp/scoop-m29-generic-primitive-{gnu,musl}-all.log`。

## Rust、公共 runner 与工具链

| 宿主 | Rust workspace | 公共 fixture runner | LLVM |
| --- | ---: | ---: | --- |
| macOS AArch64 | 43 组、5,346 项通过 | 38 项通过 | 22.1.8 |
| Linux amd64 | 43 组、5,374 项通过 | 38 项通过 | 22.1.2 |

两端 Rust 均为零失败、零忽略，`cargo fmt --all` 与
`cargo clippy --workspace --all-targets` 通过。使用 Rust 1.99，dev/test 的 opt-level=1，
保留 debug assertions 与 overflow checks。汇总保存在
`/tmp/scoop-m29-generic-primitive-workspace-results.json`，两端 workspace 和 runner 日志
沿用 `scoop-m29-generic-primitive-{darwin,linux}` 前缀。

固定工具位于各宿主主仓库的 `target/m29-generic-primitive-tools/`；Linux 两目标共用这组
工具，musl 在独立 worktree 中执行。`scoopc` 的 SHA-256 为：

```text
Darwin e1851eb17dd5671120bfe0669a31d6daa9cae8c9967c6780ab82001b06afe269
Linux  e0a27d7130e146911764af8bd4cbb69c3de6ef154505c96054d627aad6864807
```

全部三个工具的摘要记录在 `/tmp/scoop-m29-generic-primitive-tools-{darwin,linux}.json`。
Linux 目标环境和既有 LLVM libunwind 配置沿用 [M28 构建说明](../milestone28/BUILDING.md)。
本轮在 `nuc12` 的 `/home/chenxu/repos/scoop` 与
`/home/chenxu/repos/scoop/target/m29-musl-worktree` 执行，两个目标使用各自的 fixture 工作目录。
Linux 命令显式设置 `LLVM_SYS_221_PREFIX=/usr/lib/llvm-22`，PATH 包含该 LLVM 的 `bin`；
Darwin 使用 `LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22`。

后续普通全量的复现命令形式为：

```sh
python3 tests/run_fixtures.py --all --target "$fixture_target" \
  --work-dir "$fixture_work" --jobs 10 \
  --scoop "$fixture_tools/scoop" --scoopc "$fixture_tools/scoopc" \
  --scoop-link "$fixture_tools/scoop-link"
```

`fixture_target` 取上表完整 triple，`fixture_tools` 指向对应宿主的固定工具目录，
`fixture_work` 使用新的空目录。Linux musl 必须显式选择目标；切换工作目录不会改变默认目标。

后续迭代先在一个主平台完成实现与受影响测试，收敛后再做一次三平台收尾验证。前端或库
协议变更不在每批反复触发多平台全量；局部修复按实际影响范围复验，文档和已审阅快照的
同步不触发全量重跑。

## 快照、修复与格式

Darwin 核对 11,772 个实际快照，两个 Linux 目标各核对 11,654 个。三平台共有的
9,641 份快照逐字节一致，平台专有的 IR、符号、链接计划、归档成员与产物摘要分别提交。
Linux 每个目标本批更新 1,909 份快照，其中所有 JSON 仅改变摘要，所有链接计划仅改变
实际 Cone 对象数量；详细分类与原始更新、复验结果见实施记录。

全仓库回归还修复了泛型 primitive 声明保留：实际泛型实参所需的 primitive 声明随共有
HIR 保留，装箱时再按实际需求物化；`shared-bounds-context` 在三平台通过。保留产物用例
的清理改为删除实际发布根产物，避免递归 chmod 与后台 staging 清理竞争。两个 generic ODR
用例和 artifact-graph 的严格诊断仅同步实际摘要，完整独立复验通过。

协议迁移后的格式为 HIR `identity-foundation/8`、`core-bootstrap-interface/9`、
`cross-cone-interface/57`、`cross-cone-type-semantics/21`，MIR `cross-cone-type-bridge/14`，
LIR `cone-production/7`；旧产物和缓存重建，runtime metadata ABI 4 与零大小值 ABI 保持。
当前语言、分层与 runtime 契约分别见三份 [语言规范](../specs/SCOOP-SPEC.md)、
[实现规范](../specs/SCOOP-IMPL-SPEC.md)、[运行时规范](../specs/SCOOP-RUNTIME-SPEC.md)。

更新、定向复验及停止的重复运行目录均在保存报告后清理，保留既有 worktree、native
unwind、固定工具和必要构建缓存。
