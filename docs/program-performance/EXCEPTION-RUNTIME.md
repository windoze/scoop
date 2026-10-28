# 异常查找与线程内记录管理

基线 `1de3cd2eb010`。本批完成异常路径的两个局部改进：LSDA 按当前 PC 查找必要前缀，异常记录直接作为所属线程的引用根。所有 runtime profile 默认采用，无新开关。语言异常语义、Throwable 布局、generated-code ABI 与 Level I unwinder 保持；使用新 runtime 重新链接即可。

## 实现

原 LSDA decoder 在每帧中遍历完整 call-site 表，每个条目又重新解析此前条目检查重叠，并检查未命中的 action。现在利用 LLVM 按地址递增、不重叠的发射规则，按 `IP - 1` 顺序查找到命中条目或空隙便返回，只解码命中 action。保留已消费字节的边界、溢出、顺序及 catch-all 形状检查；非零 action next displacement 直接拒绝。单次查找由 O(n²) 降为 O(k)，k 为此次实际消费的前缀长度，最坏 O(n)。没有增加缓存或 PC → handler 表。

异常记录保存原 managed Throwable 的可回写引用槽，活跃链放入现有 `ScoopThreadState`。GC 在既有线程停稳协议下扫描并回写这些槽，覆盖 in-flight、caught 和 rethrowing 状态。双向链支持 O(1) 注销，包含新异常展开旧 catch cleanup 的非 LIFO 删除。begin/end/rethrow 保留 owner、状态与 catch 栈规则。

删除全局 active registry 的 mutex、成员扫描、全局计数，以及异常专用的 GcHandle 获取／解析／释放。typed throw 不再为已保证有效、已发布的引用重复获取 heap／roots 锁查询归属，仍检查非 null 与 FixedObject 形状。按 Level I 的一次性 cleanup 契约释放 native record，不为释放后再次使用非法指针维护全局成员表。

每次新 throw 仍使用 `calloc/free`。系统 libunwind、dyld 查询及栈轨迹策略属于后续工作；本批不声称整个抛出路径已经无分配或无锁。其他使用者的 GcHandle 和 GC 握手保持。

## 验证

实际运行环境为 Darwin ARM64，LLVM 22.1.8。Rust 格式化／workspace Clippy、修改 C 文件的 `-Wall -Wextra -Werror` 检查、fixture runner 的 59 项公共测试通过。

- 三组原生 EH 测试通过：decoder、personality、lifecycle。覆盖长表／区间边界、只解选中 action、嵌套／重抛／替换异常、同一对象的独立记录、线程隔离及必要错误状态。
- 66 项 collector 测试闭合。既有 moving thread protocol 用例增加只由异常记录保活的对象，验证 Managed callback、NativeSafe、NativeBorrowed 下的真实移动与根槽回写。
- 43 项异常相关正式 fixture 闭合，覆盖诊断、跨 Cone、协程、native callback 与 GC。新增 thread-roots 用例在普通、强制移动及 minor GC 三种模式下验证 nested catch、finally 中的在途异常、多线程同时展开与对象字段保留。

首次验证中，一个旧 GC 用例把 worker 上限写死为 4；原始 runtime 对照同样失败，现改用已有 `SCOOP_GC_MAX_WORKERS`，单独复测通过。两个旧链接用例将默认保留符号的 debug build 与默认清除符号的独立链接比较指纹；现匹配两端符号选项，保留指纹和原有运行断言，复测通过。没有修改生产 GC 调度或链接策略。

本批未运行完整 workspace 测试和全部文件 fixture，也未执行 Linux GNU／musl 验证；新增用例已声明原有三个目标。

## 性能与空间

相同编译器、预编译系统库及 Scoop 源码，分别使用基线和修改后的 runtime，release 编译。每个程序预热一次，再执行五对交错 AB／BA，核对完整输出。下表为完整进程墙钟中位数，包含启动，单位 ms；预先创建 Throwable，以减少对象构造对异常路径比较的干扰。

| 负载 | 基线 | 修改后 | 变化 |
| --- | ---: | ---: | ---: |
| 单线程，递归深度参数 0，50,000 次 throw/catch | 250.310 | 236.815 | −5.39% |
| 单线程，递归深度参数 16，50,000 次 throw/catch | 893.015 | 874.771 | −2.04% |
| 四线程，各 50,000 次 throw/catch，共 200,000 次 | 247.358 | 209.032 | −15.49% |

四线程用例直接在 worker 循环中抛出并捕获，调用栈与单线程递归用例不同，各行只比较自身前后。小样本用于核对本次改动，不作为完整应用或其他平台的收益结论；深栈完整路径仍保留系统展开成本。

独立 C LSDA 内核同样使用 `-O2`、一次预热和五对 AB／BA，每样本查询 20,000 次。表为有间隙的有序 cleanup 表，结果逐次核对，单位 ns／查询：

| call-site 数量与命中位置 | 基线 | 修改后 |
| --- | ---: | ---: |
| 1，唯一条目 | 17.35 | 12.25 |
| 16，首项 | 1,359.25 | 10.20 |
| 16，末项 | 1,341.75 | 173.40 |
| 64，首项 | 20,656.25 | 11.70 |
| 64，末项 | 20,909.70 | 655.05 |
| 128，首项 | 83,620.35 | 11.20 |
| 128，末项 | 83,536.35 | 1,283.85 |

这组数据只说明去掉整表重复解析的局部收益，不代表完整 throw 的同比例加速。短表样本很短，其绝对数值容易波动。

本机 `ScoopExceptionRecord` 从 96 B 降为 80 B，线程状态从 216 B 增为 224 B，`_Unwind_Exception` 保持 32 B。相同 `-O2` 编译的 `eh.c` 与 `eh/lsda.c` 两个对象的 `__text` 合计从 7,020 B 降为 4,268 B，减少 2,752 B；这不是整个程序的体积比例。候选 EH 对象已无 pthread mutex、GcHandle 或 published-object 查询导入，仍含 calloc/free。没有保存可比的完整冷构建计时，不声称编译加速。

原始基线 runtime、测量源码、各轮 JSON 和验证日志保存在仓库外 `scoop-performance-results/2026-10-29/eh-lookup-thread-state-013228/`。
