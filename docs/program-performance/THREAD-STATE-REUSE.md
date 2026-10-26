# 线程入口复用已校验状态

本项从 `0da3803e4` 开始。当前 strmap 的 native 边界采样仍包含明显的 TLS 与线程入口成本：caller-root push/pop、native 进入等先调用 `scoop_thread_current_required`，紧接着又由 `scoop_thread_require_managed` 读取同一个 TLS。检查本身有必要，重复取址没有必要。

## 采用方案

让已有的私有 managed 检查返回其已取得、已检查的 `ScoopThreadState *`。原先相邻调用取址和检查的 18 处线程、根、异常及初始化操作直接消费该结果，保持 attachment、mode、managed depth、边界、根链和 LIFO 检查及其内存序。只有原先需要当前状态的入口复用返回值；仅需检查的调用者仍可忽略它。

这是对一次同步 runtime 入口内的取址复用，不向 managed 函数增加线程参数、寄存器约定、TLS 或缓存字段；不跨 detach、异步恢复或其他线程共享状态。native enter/leave 与 caller-root publication 仍为独立步骤，NativeSafe RETURNING 握手和 NativeBorrowed 停稳协议不变。公共 runtime／metadata ABI、GC roots 和对象存活范围保持。实现统一用于全部 runtime profile，不新增诊断或测量代码。

## 验证范围

本机和 Linux 先完成格式检查、相关 C 的 `-Wall -Wextra -Werror` 语法检查及 `scoop-codegen` 的全 target Clippy，再各通过 4 项 runtime 测试：NativeSafe 受控交错、移动 GC 的线程协议根、跨 GC 的初始化成功／失败等待，以及 EH 生命周期。NativeSafe 测试包含 O0／O2 与 full／minor 四种配置，异常测试的私有 thread stub 同步返回真实测试状态。

Darwin ARM64、Linux GNU／musl 各通过 `native-scoop-abi-moving-roots`、`native-tls-cross-cone-callback-threads`、`thread-gc` 三组正式用例，共 6 个构建变体、26 个进程。覆盖 NativeSafe／NativeBorrowed、跨 Cone 与独立重链接、callback／foreign thread、debug／release 和 moving stress；没有运行完整验收。Linux 临时 sysroot 补齐线程库用例实际需要的 platform 源码后继续未执行的用例，不修改 fixture 规则。

普通运行对照沿用六个实际程序及相同系统库，基线为 `0da3803e4`，只重建并替换 runtime。每程序一对预热、三对交错 AB／BA；NUC 固定 CPU 4。另复用已有 C FFI benchmark、独立 native callee 和实际 Scoop callback，测单线程 C scalar／NativeSafe DirectC／GCLeaf DirectC；将短样本延长为每次 2000 万次调用，一对预热、三对正式交错样本。原始数据、汇编及准备日志保存在仓库外 `scoop-performance-results/2026-10-26/thread-state-reuse/`，临时构建留在忽略的 `tmp/program-performance/native-thread-state/`。

## 测量结果与默认策略

下表为普通进程墙钟中位数相对基线的变化，负数表示更快；三对样本用于本项取舍，不作为统计显著性结论。

| 程序 | Darwin ARM64 | Linux GNU | Linux musl |
| --- | ---: | ---: | ---: |
| trees | −0.18% | +1.30% | −1.25% |
| strings | −1.98% | −3.45% | +0.30% |
| strmap | −6.78% | −3.44% | +1.40% |
| dispatch | +1.68% | +1.30% | +0.53% |
| values | −0.41% | −0.01% | −0.16% |
| json | +4.02% | −0.33% | −0.81% |

NativeSafe DirectC 每次调用从 15.961／20.191／21.177 ns 降到 12.838／19.502／20.532 ns，即 −19.57%／−3.41%／−3.05%。GNU／musl 的 C scalar 与 GCLeaf 对照变化不超过 0.001 ns。Darwin 的两条对照分别增加 0.249／0.267 ns；实际 native callee 指令相同，布局从同一缓存行移到跨行，但未通过干预实验归因，不将回退抹去。JSON 为 3～8 ms 的短进程，不据此推断稳定改善或退化。musl strmap 的 1.40% 回退保留。

三目标机器码均确认 caller-root push／pop 与 native enter 省去相邻的 `current_required` 调用，只消费 `require_managed` 的返回值。Darwin 中两 helper 原先各执行一次 TLV 间接取址；一次带 caller-root 的 native 进出由七次降至四次。此计数不包括 native callee 自身或其他 runtime 操作。

七程序完整文件大小保持。Darwin text 各减少 164 B；GNU 六程序各增加 64 B、FFI 不变；musl 五程序各减少 64 B、trees／FFI 不变。线程状态仍为 216 B，caller-root frame／transition／anchor 仍为 24／64／32 B；没有对象、GC roots、静态元数据或分配行为的增量。Scoop 正文及依赖保持，consumer 仅重新链接；runtime 冷构建墙钟变化 +0.05%／+0.65%／−1.52%，CPU 变化 −0.27%／+0.93%／−2.12%，峰值 RSS 最大增加 0.38%。

采用这项低成本复用，所有 runtime profile 默认启用，无新开关。它改善重复线程取址，未改变普通 poll、Context 绑定或跨调用 TLS 策略；后续按新的 runtime 可见路径清单继续优化。
