# M32 验收记录

日期：2026-10-07。状态：已完成。设计、源码定义、编译器实现、Darwin 全部适用回归及 Linux GNU/musl 的定向验收均已完成。范围与契约见 [DESIGN.md](DESIGN.md)。

## 1. 交付

`sysroot/lib/scoop.core/src/roots.scoop` 正式声明 `@Intrinsic("core_any") public abstract class Any {}` 和 `@Intrinsic("core_nothing") public final class Nothing {}`。两个声明随普通 core 构建进入源码绑定、nominal identity、HIR/MIR/LIR、`.slib` 和最终链接；删除了生产代码中缺少 Any 源码声明时的固定身份、声明和布局兜底。

Any 保持顶类型、装箱、identity 与无成员语义。Nothing 具有实际 class identity，是所有类型的子类型，但不产生正常值；结果合并、预期类型、泛型与默认值、函数值及动态关系、严格求值、异常/finally、Context 和挂起均保留这个事实。MIR 在 Nothing 求值后终止正常 CFG，不补目标类型值；runtime 使用不可分配的 AbstractRef TD 和 Bottom 关系。

产物组合把两个根移到另一 package，由根 package 的 public alias 导出，重建 core 后移走源码，完成 provider → facade → consumer、再次发布、普通/泛型默认值、独立链接以及 normal/moving 运行。消费者依赖实际声明，不依赖根类型的固定 package 或缺失声明兜底。

兼容版本随实际编码迁移：HIR bootstrap/interface/type-semantics 为 12/61/23，MIR type bridge 为 16，LIR layout ABI/cone production 为 11/11，runtime metadata ABI 为 6，RuntimeAbiContract 为 10。旧 core、依赖与缓存需要重建；完整对照表见设计第 5 节。

## 2. 环境与方法

| target | 实际执行环境 |
| --- | --- |
| aarch64-apple-darwin | Apple M3 Ultra，32 核、512 GiB；LLVM 22.1.8 |
| x86_64-unknown-linux-gnu | `ssh nuc12`，`~/repos/scoop`；Core i7-12700H，Linux 7.0.0-38，LLVM 22.1.2，GCC 15.2.0 |
| x86_64-unknown-linux-musl | 同一 NUC，使用已安装的 musl-gcc 和对应 unwind |

先在 Mac 完成实现及新增功能的编译运行，再同步固定代码到 NUC。三个配套命令均使用独立的 release 工具副本，活跃 fixture 期间不替换二进制。Scoop 程序自身的 debug/release 是另外的测试变体，两种均实际执行。

Mac 只启动一次完整文件 suite，保留其通过结果，后续仅选择未通过项。快照生成不计入普通模式验收。相同的导入索引变化使用已观察到的一一映射迁移，完整结构差异取自实际 dump；GNU/musl 的公共 core 符号增量来自各自真实程序的 nm 输出，目标专属差异单独保留。没有修改公共 runner、增加 M32 专用执行路径或重跑 Linux 全量。

## 3. 已完成的检查

- Mac Rust workspace 首轮执行 5,328 项，5,121 项通过、207 项失败；失败集中在旧 Any 测试数据、格式版本、固定向量和相关契约断言。旧的“Any 无需声明”断言改为必须存在源码声明，修订后按 crate 或具体测试复验，当前测试覆盖已全部闭合；四个文档编译入口也已独立通过。没有忽略测试。
- Mac 公共 Python fixture runner 44 项通过。Mac 与 NUC 的 `cargo fmt` 和 workspace `clippy --all-targets` 均通过；收尾时再次通过 Mac 格式检查、workspace clippy 和 `git diff --check`。
- M32 在三个 target 各有 42 个普通模式通过用例、44 个变体、71 个正式进程和 44 次阶段快照检查。覆盖 9 个正向组合、17 个语言 negative 和 16 个 core 声明 negative，包含成员/getter、构造、值/引用、Option/数组、动态函数、协程、GC、源位置诊断及产物闭环。
- Darwin 当前发现 2,601 个 fixture，全部 2,594 个适用项普通通过，另 7 项按平台不适用。去重后的计数为 2,707 个变体、12,717 个正式进程和 11,403 次阶段／链接计划快照检查，见 [Darwin 报告](M32-DARWIN-ACCEPTANCE.json)。每个用例只采用最后一次普通模式结果，快照更新和重试不重复计数。
- NUC 的 runtime metadata 定向 Rust/C 检查 36 项通过。GNU/musl 分别完成 67 个用例、86 个变体、330 个正式进程和 449 次快照检查；范围包括 M32、泛型推断／动态函数适配／泛型 helper、HIR 默认值消费，以及 native 直接链接、archive/object、重导出、回调／异常／TLS、C ABI／Scoop ABI、GC／handle／storage 组合。见 [GNU 报告](M32-LINUX-GNU-ACCEPTANCE.json) 与 [musl 报告](M32-LINUX-MUSL-ACCEPTANCE.json)。报告中的 `complete=false` 表示未执行完整 suite，`selection_complete=true` 表示所选范围已闭合。

Darwin 首轮完整 suite 的 2,600 个用例中，1,575 项通过、1,017 项失败、7 项按平台不适用，另有一个旧产物截断长度导致的配置错误。新增的成员组合另行普通通过，因此当前总发现为 2,601 项。后续保留已通过项，只复验失败与新增项；最终报告聚合普通模式结果，已没有未通过项。

旧快照的变化包括 core 导入索引、实际根声明与控制流、符号列表以及完整链接计划中的记录表和 image 指纹。两个并发压力下超时的用例降低并发后已通过，保持原有 120 秒单步时限。损坏产物用例按新 `.slib` 的实际长度更新为截去最后一个字节，并通过损坏诊断及原程序保留断言。没有放宽源码诊断、退出状态、普通输出或程序运行检查。

## 4. 复现

```sh
export PATH=/opt/homebrew/opt/llvm@22/bin:$PATH
export LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22
cargo fmt --all
cargo clippy --workspace --all-targets
cargo build --release -p scoop -p scoopc -p scoop-linker --bins
python3 -m unittest discover -s tests/fixture_runner/tests
python3 tests/run_fixtures.py --filter 'm32-*' --jobs 4 \
  --scoop target/release/scoop --scoopc target/release/scoopc \
  --scoop-link target/release/scoop-link
```

Linux 在 NUC 上先载入 `~/.cargo/env`，将 LLVM 路径改为 `/usr/lib/llvm-22`，分别传 `--target x86_64-unknown-linux-gnu` 和 `--target x86_64-unknown-linux-musl`。完整 Darwin suite 使用 `--all`；对已有失败列表使用重复的 `--filter` 定向复验。每个选择使用独立工作目录，可复制已完成的普通 cache 目录复用产物。
