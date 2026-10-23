# 按需 MIR 优化决策报告

本项落实调研 §7.1 的优化诊断，独立于 PGO。直接消费者是上一项真实 ArrayList 遍历：定位 `next()` 时需要临时打印内联成本，已有 MIR dump 能显示结果，却不能解释为何选择或保留。首批将这类信息做成可缓存的按需观察输出。

## 设计

- `scoop build --emit optimizations --dump-dir DIR` 输出每个所选源码 Cone 的 `optimizations.json`；独立 `scoopc build --emit optimizations` 沿现有 emit 输出。`--emit all` 包含报告。
- 沿用现有 dump 选择、cache、scope 与 observation-only 流水线。新增 stage tag 5，排序在 MIR 后、LIR 前；machine protocol 升至 15，既有 stage tag 保持。语言、runtime、接口表布局和 `.slib` 不变。
- MIR crate 仅定义本机观察数据，mir-lower 在已有决策点按需填充，driver 复用当前 Cone 与已加载依赖的 HIR 声明来源渲染 JSON schema 1。报告不进入语义 Module 或 metadata，不引入新的跨 stage 实现依赖。
- 内联保留实际使用的成本与选择原因，包含普通估价和局部 class 字段替换的共同估价。分配消除复用已有合法性与使用分析；不为诊断重跑优化，不改任何阈值。
- 保存实际迭代、观察坐标和内联来源；关联函数身份及已有声明来源。源位置缺失时明确缺失，观察坐标不等同于最终 MIR 地址。首批范围是普通 MIR 内联和局部分配消除，不声称已有 CPU 采样或 PGO。

## 使用与范围

```sh
target/release/scoop build tests/fixtures/driver/stage-observations/main.scoop \
  --release --scoopc "$PWD/target/release/scoopc" --sysroot "$PWD/sysroot" \
  --emit optimizations --dump-dir "$PWD/tmp/optimizations"
```

输出位于 `dump-dir/ConeIdentity/optimizations.json`。默认只观察 root，`--dump-scope sources` 沿用源码节点选择；独立 `scoopc build --profile release --emit optimizations` 沿现有 emit 路径输出到 stdout。当前 profile 下未启用的普通 pass 不产生失败事件。

`functions`／`external_callables` 保存临时 MIR 引用对应的 materialization、implementation、精确参数／结果与可用的声明来源；同名泛型实例必须结合这些身份和签名区分。`events` 保存实际决策顺序、轮次、当时的 block／statement、span 和 `inline_path`。位置不是最终 MIR 的稳定地址，声明来源缺失时为 `null`。内联后的节点关联原正文；不会将 imported／inlined span 一律归到 caller 文件。

内联的 execution、size、growth 是既有 MIR 策略权重，不是实测 CPU 时间。`execution_limit = null` 表示此前的增长检查已拒绝，或必需展开绕过了该检查；`class_projection` 区分未尝试、证明不足和实际估价。分配消除报告复用已有来源／循环／支配／使用计数，不增加另一套逃逸分析。`rounds`／`converged` 描述原有局部优化主循环；首批逐项事件只覆盖内联与分配消除，不覆盖前置必需 intrinsic 规范化、后置 poll chunking 或 LLVM passes。

## 真实消费者

[classes.scoop](../../tests/benchmarks/allocation/classes.scoop) 的三个目标均产生 189 条事件，报告文件 197,523 bytes。`lists` 中 `ListIterator<Long>.next()` 的原始成本为 71；参数简化后的普通成本为 67，需增长 61，而 caller 仅剩 27。与字段替换共同估价后，执行／体积成本为 29，执行阈值为 44，增长 23，因此报告 `inlined-with-class-fields`。接收者为精确 `ListIterator<Long>`、返回值为 `Option<Long>`；来源正确关联到 core 的 `src/iteration.scoop`，独立于显示名 `next`。

这替代了上一项调查中的临时成本打印，并与已有的 iterator 分配消除结果闭合。报告同时如实显示此程序使用了 3 轮，尚未观察到完整的无变化轮次；没有因此改变轮次上限或优化选择。

## 定向验证

- 本机覆盖 51 项相关 MIR 优化用例，包含报告开关前后的 MIR 一致性、真实共同估价、关闭 pass、逃逸／混合来源／异常控制流／循环别名／支配保留，以及嵌套内联的原正文 span。协议 13 项、观察缓存 1 项和 driver 停阶段专测通过；Rust 格式、workspace clippy 和固定版本 Ruff 通过。
- 三目标各运行 `stage-observations` 一组正式 CLI 用例，每组 13 个进程步骤。检查缺失报告只补观察、机器产物／编译记录未重写、随后 `all` 直接复用、源码变化后更新；同时核对泛型依赖正文及 external callable 的实际来源。源码和工具始终存在。
- CLI 暴露的响应端旧四阶段映射已移除，请求／响应复用同一 tag 解码；五种观察的响应往返及上限已覆盖。现有测试的缓存观察路径由旧 `compile-v2` 修正为当前 `compile-v3`，并要求实际找到产物。
- driver 的 Rust 专测在默认测试线程栈上溢出；显式使用 `RUST_MIN_STACK=33554432` 后通过。正式 release CLI 未调整线程栈。没有运行完整 fixture 或 workspace 测试验收。

## 代码与空间

基线为 `c78aa041c`。三个目标分别比较基线、当前关闭报告、当前开启报告：LIR、`.slib` 和完整程序在各目标内部均逐字节相同；Point／Cursor／ArrayList／scalar 四种运行输出也与已知结果一致。静态表、机器代码、GC 信息与运行期空间没有增加；报告是独立的本机文件。

| 目标 | `.slib` bytes，三形态相同 | 完整程序 bytes，三形态相同 |
| --- | ---: | ---: |
| Darwin ARM64 | 858,546 | 18,082,920 |
| Linux GNU | 1,064,822 | 17,630,552 |
| Linux musl | 1,065,080 | 17,026,600 |

## 编译成本

复用已有 `process_metrics.run_process`，单独测量两个对照：基线与当前关闭报告、当前关闭与开启报告。每组一对预热、五对 AB／BA；直接运行 `scoopc` 编译相同 class 基准并消费预编译 std。以下为中位数，时间包含实际生产与输出，未从数据中扣除格式化或 I/O。各样本的 `.slib` 均相同。

| 目标 | 默认编译 ms：基线 → 当前 | 时间变化 | 默认峰值 RSS 变化 | 开启报告 ms：关闭 → 开启 | 时间变化 | 开启报告峰值 RSS 变化 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Darwin ARM64 | 1,093.331 → 1,096.850 | +0.32% | −0.86% | 1,097.245 → 1,094.924 | −0.21% | +1.75% |
| Linux GNU | 1,519.137 → 1,539.416 | +1.33% | +0.09% | 1,546.355 → 1,544.481 | −0.12% | −0.01% |
| Linux musl | 1,523.344 → 1,539.408 | +1.05% | +0.09% | 1,540.364 → 1,540.269 | −0.01% | +0.00% |

保留默认编译的实测增加；显式报告的时间差接近本组样本波动，不能由这些负差推断报告带来加速，也不能外推到大型 Cone 的完整诊断开销。报告继续按需启用，普通路径不保存逐项事件；现有 release 优化策略保持。本项提供静态优化调查工具，CPU 采样、其他 pass 诊断与 PGO 仍按各自实际需求推进。

环境沿用上一项：M3 Ultra／macOS 27.0.1／LLVM 22.1.8，nuc12 i7-12700H／Ubuntu 26.04.1／LLVM 22.1.2。按本任务约定，原始脚本、产物、日志与样本位于两机 `tmp/program-performance/optimization-report/`；仓库只保留本概要。
