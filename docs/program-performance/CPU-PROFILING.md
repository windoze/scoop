# CPU 采样与函数级符号归因

本项从 `dcac9e457` 开始，补足测量主线的函数级归因。使用平台采样与 LLVM 工具，程序不消费 profile；命令与限制见[使用说明](../../tests/benchmarks/PROFILING.md)。

## 采用方案与开启条件

- 所有 profile 可显式 `scoop build --emit symbols --dump-dir DIR`，`--emit all` 也包含；独立 `scoopc build --emit symbols` 输出同一 JSON。未请求时不收集映射，release 不自动采样。
- LIR 完成后输出 schema 1 的 symbols.json，复用当前 Cone 及已加载依赖的 callable-body、完整 exact signature、模板／实例与 HIR 声明来源；不另读依赖源码或重放语义检查。显示名不参与身份判断，重载、泛型实例与 adapter 保留各自身份，缺失来源记 null。
- 复用既有观察缓存：缺失 symbols 只补到 LIR，已有机器产物和最终链接保持；再次请求直接复制观察。新增 stage tag 6、machine protocol 16，`.slib`、语言身份、对象、GC 与 ABI 不变。
- `symbolize.py` 只注释已知符号，保留原名、偏移、权重与调用树。给出 `--binary` 后，用 LLVM nm 关联与唯一已知正文同址的定义起始标记；多个正文同址时不猜测。
- macOS sample 的 `<deduplicated_symbol>` 通过显式二进制、实际主镜像加载地址、LLVM readobj 与 symbolizer 解析。只附加报告明确列出的 PC 及函数，保留折叠计数和省略号，不重新分配样本。
- release 用 `--debug-info line-tables` 生成 DWARF 及相邻 dSYM／ELF debug file；`symbolize.py --binary` 通过 LLVM 的结构化输出关联地址、源文件／行和内联帧，再以精确 linkage name 查询 symbols 观察。Darwin sample 自动计算实际 ASLR slide；`--address` 默认接受链接地址，Linux 运行 PC 需显式提供实际 `--load-bias`。旧 map 选项和 parser 已移除。

symbols JSON 是逻辑观察，不是最终链接存活清单。声明 byte span 不是执行行号，指令位置由 DWARF 提供；仅有可读名称的内联帧不按名称合并实体。LLVM 克隆、native／runtime 及未知符号保持原始信息。部分 perf 版本给边界别名显示异常偏移，工具保留该值，不将其当作合法 PC 偏移。

下文是最初函数级观察实现的测量记录；调试信息及 map 迁移的验收见[调试实现记录](../debugging/IMPLEMENTATION.md)。

## 定向验证

- 本机 Rust 格式化、两机 workspace Clippy 通过；观察／缓存／文档协议的 12 项定向 Rust 测试通过。编译入口测试以 `RUST_MIN_STACK=33554432` 运行；第一次使用 Rust 测试线程默认栈时溢出，调整测试环境后通过，产品栈设置未变。
- 两机各 9 项 Python 测试通过，覆盖完整符号边界、平台下划线、未知／克隆符号、重复映射、同址歧义、ASLR、折叠地址与原始计数保留。
- Darwin ARM64、Linux GNU／musl 各通过 `stage-observations` 一组正式 CLI fixture，每组 15 个进程步骤。覆盖同名重载、两个 generic 实例、boxed adapter、预编译依赖、声明及 signature 对应、缺失观察单独生成、机器产物不重写、缓存命中与源码更新；debug 观察由 Rust 入口用例覆盖。
- 三目标分别比较旧工具、当前默认、当前 `--emit symbols`：引用数组基准的完整 `.slib`、可执行文件逐字节一致，代表字段求和、接口复制与父投影输出一致。最终程序中各 2,181 个 Scoop callable body 均可由映射关联。没有全量 fixture 或重复 GC 验收。
- 三目标实际采样均恢复 `main → exercise → sumNodes`／`projectInterfaces`。GNU／musl 各四份 perf 报告含 402～403 个 cycles 样本、丢失数为 0；macOS 的同址折叠问题及 GNU 的定义边界别名问题已在真实报告上闭合。

## 编译与空间成本

使用相同 release 引用数组源码、预编译 sysroot 与工具链。每种配置独立编译六次，首轮预热不计，余下五次取中位数；配置次序交错。下表每格依次为墙钟／CPU／峰值 RSS 的变化。

| 对照 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | --- | --- | --- |
| 当前默认相对旧工具 | −0.38%／−0.19%／−0.01% | +0.25%／+0.24%／+0.17% | +0.88%／+0.49%／+0.16% |
| 开启 symbols 相对当前默认 | +1.21%／+1.26%／+4.09% | +1.31%／+1.33%／+0.48% | +0.87%／+0.95%／+0.54% |

每份 symbols.json 为 2,713,202 bytes，保存在观察目录；没有增加程序机器码、静态表、GC roots 或 `.slib` 大小。默认编译的小幅差异属于本组观测，不作为加速或显著回退的结论。另一个包含 ArrayList 预编译依赖的正式用例输出 2,092 条记录、2,591,346 bytes。

## 采样扰动与实际用途

临时输入仅将引用数组基准长路径的轮数从 128 增至 262,144，长度固定 65,536；原基准源码不变。字段求和执行三对普通／采样交错对照，stdout 一致；另采集一次接口父投影。每次只采样 2 秒窗口，程序总内核约 5～7 秒，以下不是全程采样的通用成本。

| 目标与采集设置 | 目标内核墙钟变化 | 目标内核 CPU 变化 |
| --- | --- | --- |
| Darwin，sample 1 ms | +2.64% | +0.80% |
| GNU，perf cycles:u 199 Hz、frame pointer | +3.82% | +0.97% |
| musl，相同 perf 设置 | +2.44% | −0.45% |

CPU 来自目标程序的阶段时钟，未计入采样器自身资源；musl 的小幅负差不代表采样加速。目标峰值 RSS 分别变化 +0.12%、0、0。Linux 在已有 `sudo -n` 权限下 attach 到普通用户进程，未修改系统配置；普通 A/B 计时继续关闭采样。

在此次窗口中，Linux 字段求和的 cycles 集中于 sumNodes；GNU／musl 父投影分别有 99.74%／100% 归于 projectInterfaces。macOS 明确列出的 sumNodes PC 对应数组引用加载与对象字段加载，projectInterfaces PC 对应局部引用重读和双字写入；结合反汇编，可继续聚焦这些内层操作。sample 的折叠总计数不能据此全部分配给某条指令，也尚不能解释此前所有布局敏感回退。

原始样本、命令、编译产物及日志保留在两机忽略的 `tmp/program-performance/profiling`；文档只记录范围、结果与限制。行表和内联归因现已由调试信息提供；后续按具体热点选择 PMU 或 pass 诊断，PGO 仍是独立后续能力。
