# M35 验收记录

2026-10-10 实现与验收完成。设计与实施明细见 [DESIGN.md](DESIGN.md) 和 [PROGRESS.md](PROGRESS.md)。

基线为 `75a534ef2`；功能实现截至 `38c75215a`，本机回归迁移提交为 `8400796d6`。按用户要求，先完成本机实现、验证和提交，再把同一提交同步到 `nuc12:~/repos/scoop`。Linux 验证使用分支 `codex/m35-verification`。

## 交付能力

| 能力 | 实际行为与覆盖 |
| --- | --- |
| 显式 case | 模式分支使用 `case pattern`；普通表达式不再被猜测为绑定模式。迁移 core、JSON 库、历史源码及动态测试输入，保留原有模式、guard 与覆盖规则。 |
| 普通 when | 支持有／无 subject、头部 val 与解构声明、普通值、is／in、多条件短路、guard、guarded else，以及普通条件与 case 混合。subject 只求值一次，语句落空与值位置穷尽性分别处理。 |
| return／throw 与 Elvis | 跳转可处于表达式位置，并检查合法返回目标；Elvis 只合并正常完成路径的类型。终止路径不伪造结果 local，保持求值顺序、finally、Context 和挂起清理。 |
| 尾随 lambda | 固定映射到每个候选的最后形参，该形参不得为 vararg；其他实参继续遵守默认值、命名、spread 和推断规则。覆盖普通／成员／extension／safe／super／constructor／alias／invoke／函数值调用与 suspend lambda。 |
| 注解数组 | annotation 参数接受合法标量的一维 core `Array<T>`，保存完整类型及静态元素值。覆盖空值、顺序与重复、默认值、alias、原始浮点 bits、静态描述查询、普通 JSON codec 和跨 Cone 再发布。 |
| smart cast 组合 | 稳定 binding 可保存多个已知类型视图，合取、替代路径与 guard 按实际控制流合并。成员、属性、扩展、invoke、callable reference 和捕获沿普通声明身份与重载规则解析；捕获保留原存储类型。 |

新增 131 个文件 fixture：case 3、普通 when 39、跳转与 Elvis 19、尾随调用 21、注解数组 39、smart cast 组合 10。每组包含独立正例、真实组合、精确位置与信息的编译错误断言，以及适用的 AST／HIR／MIR／LIR 转储。跨 Cone 正例包含 debug／release、删除源码后的产物消费和独立链接；组合覆盖异常、Context、挂起、minor／full moving GC。

## 本机完整验收

Darwin/AArch64 使用 `LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22`。每批先执行 `cargo fmt --all` 和 `cargo clippy --workspace --all-targets`，再运行相应测试；正式文件验收使用配套的 release `scoop`、`scoopc`、`scoop-link`。

Rust 工作区 5,437 项测试覆盖闭合，公共 fixture runner 44 项通过。初跑的两个失败分别来自旧尾随 lambda 的参数位置和 `case` 前缀引起的警告列号；修正后只定向复验，并补齐尚未执行的 crate 与 doctest。额外的 release driver 复验没有重复计入工作区数量，详见 [Rust 记录](validation/rust-darwin.json)。

正式 CLI 执行一次 `--all`，发现 3,065 项：初跑 2,714 项通过、335 项需要迁移、16 项按平台不适用。随后按未闭合结果执行 89、174、74、1 项，最终覆盖为 **3,049 项通过、16 项不适用、无未闭合失败**。按每项最终通过来源去重，共 3,289 variants、14,568 processes、12,407 次阶段／链接计划快照检查。131 个 M35 新 fixture 在正式全量中全部严格通过。

预期迁移保留原有诊断、退出、stdout／stderr、运行、GC 和链接断言，同时保存实际规范化输出；四批共 2,727 次观测与最终预期逐份严格一致。最后一批涉及的 35 份共用快照没有变化。完整逐项来源见 [CLI 覆盖记录](validation/cli-coverage-darwin.json)，没有把这些补验称为另一轮无更新的完整执行。

旧源码迁移继续保留测试目的：M23 的非末位 lambda 实参移回括号，M18 的泛型 invoke 将 lambda 形参放在最后并继续覆盖默认值，M8 的显式 return 改用块体函数。两个 Array 反例保留主要类型错误，并更新另一构造候选的重复绑定诊断。其余诊断迁移仅改变 `case` 引起的源码位置。

## Linux 验证范围与执行方式

Linux 使用 LLVM 22.1.2、GNU C 工具链及 `musl-gcc`。`cargo fmt --all`、workspace clippy 和三个 release CLI 的构建通过；公共 runner 的 44 项测试通过。Rust 工作区以 release 模式执行一次，43 个测试目标共 5,468 项全部通过，0 failed、0 ignored；复用配套 compiler 并关闭 incremental。平台条件测试使数量与 Darwin 不同，完整命令和结果见 [Rust Linux 记录](validation/rust-linux.json)。

Linux 最初按所有变更源码列出 600 个候选，清单保留在 [linux-selection.json](validation/linux-selection.json)。执行中确认该集合包含大量只有前端源码位置变化、已在 Darwin 完整覆盖的重复验证；MIR／LIR 没有变化的历史 core 布局也不需要在两个 libc 下逐项重新构建。因此最终必验范围聚焦 177 项：全部 131 个 M35 用例、实际 MIR／LIR 变化、Linux 专用回归，以及按新规则迁移的旧语义示例。GNU 适用 170 项，musl 适用 173 项，见 [最终必验清单](validation/linux-focused-selection.json)。

已完成的额外回归全部保留，已观察到的失败逐项补验；取消的其他候选单列，不计为通过。最终按每个 fixture 的最后完整结果去重：

| target | 通过 | 不适用 | variants | processes | 阶段／计划快照 |
| --- | ---: | ---: | ---: | ---: | ---: |
| x86_64-unknown-linux-gnu | 344 | 20 | 354 | 1,363 | 1,479 |
| x86_64-unknown-linux-musl | 306 | 17 | 317 | 945 | 537 |

GNU 的必验范围为 170 项通过、7 项不适用；musl 为 173 项通过、4 项不适用，均无未闭合失败。两者都包含全部 131 个 M35 用例。原 600 候选中，GNU 有 236 项、musl 有 277 项未完整执行且不在最终必验范围，明确保留在报告的 `excluded_incomplete_candidates`，没有将本轮称为 Linux 全量验收。逐项执行来源见 [GNU 覆盖](validation/cli-coverage-gnu.json) 和 [musl 覆盖](validation/cli-coverage-musl.json)。

musl 的 131 个 M35 用例已全部通过，共 136 variants、238 processes、146 次阶段快照检查。保存的 146 次实际观测逐份严格一致；106 份共有快照与已有预期相同，新增 40 份 musl 目标 LIR，其中 16 份与 Darwin 相同，24 份反映实际 carrier／ABI 差异。没有改写共有 HIR／MIR。专项报告与迁移明细见 [运行记录](validation/musl-m35.json) 和 [快照核对](validation/musl-m35-snapshots.json)，已在 `4cb0ef8fc` 独立提交。

CLI 执行保留全部运行断言及每份实际快照。后续批次的更新输出写入独立临时目录，只重定向 snapshot 路径，普通断言仍由原 runner 按原规则执行。最终 2,016 次实际观测、1,628 个不同仓库文件逐份一致，见 [最终快照核对](validation/linux-final-snapshots.json)。每个 Linux target 新增 40 份 M35 LIR，并迁移 19 份历史 LIR；实际 GNU／musl 输出相同，共有 AST／HIR／MIR 没有变化。迁移保留 Elvis 正常结果与局部编号变化、invoke 最后形参映射等实际结构，运行与 GC 断言同时通过。

GNU 初始 4 项因并行 Rust 构建重写 `target/release/scoopc` 而触发工具一致性检查，后续执行改用 [固定工具副本](validation/tools-linux.json)。两个 core 重建用例达到原有 300 秒时限，随后以较低负载补验。收尾时 `/tmp` 的用户配额限制影响 GNU 5 项、musl 6 项；保存全部报告与实际快照后，将工作目录和 `TMPDIR` 迁至磁盘上的 `target/m35-linux-validation`，这 11 项全部通过。原 6 项失败也全部闭合；不修改时限或断言，不重复已闭合用例。

## 格式与兼容性

| 合同 | M35 最终版本 |
| --- | ---: |
| HIR cross-cone-interface | 72 |
| HIR cross-cone-type-semantics | 29 |
| runtime ABI contract | 12，保持不变 |
| runtime metadata ABI | 8，保持不变 |

HIR 共享格式承载新的 when、跳转表达式、注解静态数组与完整参数类型；旧 profile 及相关产物／缓存需要重建。MIR／LIR ABI 沿用已有控制流、转换、调用和布局机制，没有增加用户可见的 intersection type 或运行期注解数组。实际格式向量、引用检查、profile 指纹与跨库消费已纳入验证。

## 代码长度与构建目录

新增 Rust 实现文件最长 244 行，新增测试文件最长 246 行；表达式后缀、place、尾随调用、when、跳转、类型视图和注解处理按实际职责拆分。主 parser 表达式文件从约 600 行降至 270 行，移除只服务旧 Elvis 的脱糖路径。变更没有新增 TODO、`todo!` 或 `unimplemented!` 占位。

Darwin 按批次完成六次 incremental 清理，保留有效 CLI 与依赖。Linux 清理旧 M33 临时 fixture 产物，后一次完整测量删除约 16.3 GiB，保留旧源码与注册的 worktree。配额恢复时，清理已结束批次的 case／cache／sysroot，保留报告和规范化快照，`/tmp` 占用从约 24.4 GiB 降至 1.85 GiB。磁盘补验完成后再次清理专用缓存与中间产物，target 从约 22.6 GiB 降至 16.8 GiB。目录可能共享硬链接，记录区分目录大小与清理前后实际测量，见 [清理记录](validation/target-cleanups.json)。

## 分批提交

| 交付 | 提交 |
| --- | --- |
| 显式 case 与源码迁移 | `04f10dbfe` |
| 普通 when 与 subject binding | `1a54b42fb` |
| 跳转表达式与 Elvis 正常结果合并 | `0ad1c9793` |
| 尾随 lambda 最后形参映射 | `b365e73ca` |
| 静态注解数组 | `69e08961b` |
| smart cast 调用与捕获组合 | `38c75215a` |
| 历史预期迁移与 Darwin 完整验收 | `8400796d6` |
| musl 的 M35 专项验收与目标转储 | `4cb0ef8fc` |
| GNU／musl 回归闭合、最终目标转储与清理记录 | `918365b54` |
