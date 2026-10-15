# M36 验收记录

> 原始日志、逐次 JSON 报告和压缩归档已移出版本库。本页保留验收范围、结果与限制；文中的历史报告名称仅标识当时的验证批次。

M36 已完成（2026-10-11），12 项功能分别提交，编译器输入及 IR 的完整性已按设计收紧。设计范围见 [DESIGN.md](DESIGN.md)，生产／消费路径及每项定向验证见 [PROGRESS.md](PROGRESS.md)。

基线为 `d276dcf97`。实现保持现有语言、实体身份、求值次序、异常、GC 和 runtime ABI；源码修饰符错误保留准确的 modifier span。没有引入依赖、认证或预算体系，也没有增加通用状态机。

## 交付范围

- 调用按 Direct／Virtual／Interface／Closure／FunctionBridge 保存完整目标，动态 receiver 必填；去虚拟化一次构造完整新调用。
- 有 subject 和纯条件 when 分别保存允许的条件与 fallback，覆盖信息使用 subject 自身的类型；语义 Return 必含结果表达式，bare return 在 HIR 形成 Unit，Nothing 保持终止路径。
- 最终 MIR 移除协程 pending chain、临时 callee 和未生产的 Cast；lowering 私有工作记录保存真实构造顺序，完整 MIR 只交付完成后的操作。
- 固定 intrinsic 使用完整一元／二元 payload，方法引用直接构造固定形参，数据借用携带具名 object／block 及从已决议形参获得的类型信息。
- FunctionTypeId、EnumApplicationId、declared nominal、C／Scoop struct 窄引用沿生产／消费路径保留类别；core protocol 使用八组具名 typed 字段。
- NativeBorrowed 六种返回分支一起保存调用和结果发布数据，间接结果只持有一份 storage；codegen 保留完整 callee 与 native transition。parser 合并修饰符取值／位置，AST literal pattern 排除任意表达式。

新增 26 个文件 fixture，覆盖独立行为、真实组合、准确诊断位置与各阶段输出。组合包括 generic／default／跨 Cone 再次消费，debug／release、receiver 次序、Nothing、try/finally、Context、协程恢复、C 与 Scoop ABI、native 调用及 moving／minor GC。

12 项功能分别提交，后续兼容性、回归修复与最终快照验收另行提交：

| 项目 | 主提交 |
| --- | --- |
| R01 完整调用目标与 receiver | `66b40f2b7` |
| R02 Subject／Conditions when | `6b1cd6a4d` |
| R03 lowering 临时态移出最终 MIR | `56424b576` |
| R04 固定元数 intrinsic 与具名数据借用 | `23a854e17` |
| R05 函数与 enum application 窄身份 | `8e0c48c41` |
| R06 declared nominal 与 C／Scoop 窄引用 | `25d83d63d` |
| R07 完整语义返回值 | `02c3df7ca` |
| R08 NativeBorrowed 调用与结果发布 | `bd39e23ba` |
| R09 完整 codegen 调用准备 | `4d16aa907` |
| R10 具名 core protocol | `04b8cfcd8` |
| R11 修饰符取值与位置 | `fbb5f9028` |
| R12 封闭 literal pattern | `b87b0225a` |

## 验证方式与已闭合回归

每批先 fmt／workspace clippy，再运行受影响的 Rust 测试和正式 CLI。实现期间保留真实运行、stdout／stderr、退出、链接与 GC 断言。golden 先生成和审阅，再由普通比较核对；最终 Linux 执行一次完整 workspace Rust 和正式 CLI `--all`，本机不重复无关全量。

两个最终回归已单独修复：Linux metadata 测试补上 R06 struct definition 投影；默认参数中的穷尽 when 在 R02 重构后漏收集外层 origin 的 subject 类型引用，现从完整 subject 直接记录，新增 enum／Boolean／wildcard 真实源码回归。后者修复前后 MIR／LIR 相同，HIR 仅补齐引用站点，没有增加重复类型字段或校验机制。

## 最终验证

下表只统计普通比较，快照生成不计作最终普通验收。GNU 使用一次正式完整 `--all` 加失败项补验；Darwin 和 musl 使用针对本次变化的选集。三个目标均包含全部 26 个 M36 新用例。

| 目标 | 验证范围 | 通过／不适用 | 配置 | 进程 | 阶段／计划快照 |
| --- | --- | --- | --- | --- | --- |
| `aarch64-apple-darwin` | 本机主选集及补充回归 | 236／0 | 342 | 2,524 | 2,337 |
| `x86_64-unknown-linux-gnu` | 全部 3,091 项，逐项合并全量与补验 | 3,061／30 | 3,305 | 14,478 | 12,256 |
| `x86_64-unknown-linux-musl` | 188 项受影响与 musl 专项 | 188／0 | 291 | 1,913 | 1,525 |

本机的 236 项为原 231 项主选集加五项补充回归；五项 target 可移植性复验与主选集重叠，不重复计数。来源见 Darwin 普通覆盖、GNU 最终覆盖 和 musl 普通结果。musl 的生成与普通比较分开保存，普通比较为 188/188 直接通过。

GNU 初次全量为 2,855 项通过、206 项失败、30 项不适用。192 项为阶段快照迁移，13 项构建超过原有 300 秒时限，另有一项 `override release` 的旧诊断预期仍使用空区间。审阅实际转储并更新预期后，以 6 并发普通补验 206 项，205 项通过；嵌套 enum 的第三层消费又发现一份旧 LIR，核对一致局部编号映射后仅复验该项，通过全部后续步骤。13 项超时均在原有时限内通过，没有放宽断言。保留 初次摘要、完整原始报告、首轮补验 和 最终单项补验。

Linux 完整 `cargo test --workspace` 正常退出，43 个测试目标、5,474 项通过，0 failed、0 ignored。最后的默认参数引用修复另以相关 29 项通过，其中新增一项，最终不同测试覆盖为 5,475；该数字不表示在最后一次修复后重跑了整个 workspace。两机最终源码的 fmt／workspace clippy 通过，公共 fixture runner 各 44 项通过，实际命令和版本见 Linux Rust 记录。各功能的本机定向 Rust 和 CLI 记录见 [PROGRESS.md](PROGRESS.md)。

本机旧阶段快照分七批处理 1,237 项，最终为 1,236 项通过、1 项不适用；默认参数引用问题的初败及普通补验来源单独保留。随后用上述普通选集及 GNU 正式全量检查结果，避免再次重复本机无关全量。详见 阶段迁移。

## 快照与兼容性

HIR／MIR 更新反映完整调用、when、intrinsic 和 Unit Return 的表示。LIR 差异以无用零大小值物化及临时编号变化为主；审阅另外确认 Unit 字段提取、无用 `MaybeUninit<Unit>` 结果和协程 Unit 保存／恢复的删除。`coroutines-closure-finally` 的一个私有 lambda frame 从 88 缩为 80 字节，字段和 scan offsets 同步迁移；`coroutines-failure-finally` 保留初始 Unit 参数槽，只删除无用保存／恢复及空跳转。

嵌套 enum 默认参数的 GNU／musl LIR 还包含局部变量分配顺序变化；已逐一核对定义、使用、GC root 与生成调试名的一致置换。各平台逐份审阅见 Darwin、GNU 和 musl。

新 C 布局用例对刻意低对齐的嵌套声明局部屏蔽 GCC packed-not-aligned，保留其他 -Werror 检查；含真实 ABI 差异的九组 LIR 使用目标专用文件，两机 C 语法检查与三个目标普通 fixture 均已通过。

现有 section 编号／版本及 runtime ABI 不变。旧 wire 的可空 Return 在绑定时形成真实 Unit，新 writer 使用已有的 Present 分支写出完整返回值；这不表示 M36 的所有产物都与基线逐字节相同。整数与 when 在原解码边界组合为完整分支。R10 的实际 core 与两份用例产物共 3,265 个非 manifest archive 成员和基线逐字节相同。

## 文件长度与构建目录

新增 49 个 Rust 文件，最长 396 行；原有超过 500 行的文件没有增加超过 30 行。模块按实际职责拆分，不为行数机械切割。已有语义与布局边界检查继续保留，见 长度检查。

所有构建与测试的临时目录位于仓库 `target/m36-tmp`，没有使用 `/tmp`。共清理 13,938 个过期 Rust 中间对象、过期 incremental 缓存，以及各批已通过的 fixture cases；GNU 失败目录在相应普通补验通过后清理。最终两个任务临时目录均为空，本机 target 实际占用约 19.1 GB、Linux 独立工作树约 19.5 GB，保留当前 CLI、可复用依赖、sysroot 与报告。逻辑清理大小与实际磁盘占用分开记录，见 清理及原工作区检查。

Linux 使用从 `nuc12:~/repos/scoop` 创建的独立工作树 `~/repos/scoop-m36-validation`，原目录已有未提交改动保持原样；M37 原有设计稿未纳入本次提交。
