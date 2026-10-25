# 启动时合并 site 的 owner 验证

相对 `6515799cd`，将每个 safepoint 的 owner 解析与验证合并到首条 stackmap record 的处理。原先建立 site 索引时按完整 callable ID 查找一次，处理每条 raw record 时再次查找；现在只在该 site 首次合并时查找一次，同时检查归属、entry、root count 和目标 frame。重复 ODR record 比较已验证记录的地址与完整规范化 payload，复用 owner 和 frame 验证结果。

收集阶段仍检查 site ID、role、root count 与唯一性。缺少 owner、归属不符、缺少 raw record、错误 root／frame、重复 Strong、ODR 内容不一致及重复 return PC 均在 managed 执行前拒绝。该实现只改 `image/types.c` 和 `image/stackmaps.c`，没有新增缓存、登记字段、测量代码或对象／GC roots；不改变 ABI、`.slib` 和预编译 Cone 的合同。

采用新版 runtime 后，全部 profile 默认启用，无独立开关。新增启动测量仍仅由 `--enable-startup-stats` 编入，关闭时预处理排除。

## 定向验证

- 两机格式检查、workspace Clippy，以及四个受影响 C 源文件的 `-Wall -Wextra -Werror` 检查通过。各运行四项现有 image 测试，覆盖 registry、类型／扫描、storage／初始化与 stackmap join。
- 既有 stackmap 用例现有 21 个失败组合。补入缺少 owner、不同 ODR 归属及错误重复 frame；三个 raw record 合并为两个 site 时，实际目标 frame 验证只执行两次。类型测试补上 root count 越界，owner 失败验证移到实际 join 边界。
- 三目标的同一组 `compiler-correctness/startup-stats` 通过：debug／release 共 6 个 variant、54 次外层进程调用。覆盖普通／测量／stress 组合、跨 Cone 启动、eager 与 main 顺序、两个链接入口、开关切换及不匹配配置拒绝；没有全量验收。
- 检查两个实际程序的默认与统计版机器码：`scoop_image_validate_code_and_types` 中的 owner lookup 调用点从 1 变为 0，`scoop_image_stackmaps` 保留首个 site 分支中的一次调用。默认最终产物无启动测量报告、计时错误字符串或专用计数查询符号。

## 启动性能

M3 Ultra／LLVM 22.1.8 与 NUC i7-12700H／LLVM 22.1.2；Linux 固定 CPU 4。复用已有最小输出程序和通过 `scoop.json` 解码 Int 的程序，两者输出均为 `7`。每组 3 次预热、31 对交错 AB／BA，测量时不并发构建。下表是**关闭启动测量**的完整进程墙钟中位数，单位 ms：

| 目标 | 最小程序：之前 → 之后 | 降低 | JSON：之前 → 之后 | 降低 |
| --- | ---: | ---: | ---: | ---: |
| Darwin ARM64 | 10.566 → 9.880 | 6.49% | 12.148 → 11.281 | 7.14% |
| GNU x86_64 | 9.761 → 8.971 | 8.10% | 11.675 → 10.750 | 7.92% |
| musl x86_64 | 24.932 → 24.270 | 2.65% | 29.754 → 28.907 | 2.85% |

六组各 31 对样本中，候选均快于基线。这是代表程序的启动收益，不是稳态吞吐或综合跨语言性能结论。

另用相同统计配置交错测量阶段成本。以下各列分别取中位数，单位 ms；总量只覆盖 runtime 入口至 eager 结束：

| 目标／程序 | 代码与类型：之前 → 之后 | stackmap：之前 → 之后 | runtime 启动：之前 → 之后 |
| --- | ---: | ---: | ---: |
| Darwin 最小 | 1.757 → 1.011 | 2.295 → 2.323 | 6.241 → 5.502 |
| Darwin JSON | 2.110 → 1.203 | 2.769 → 2.819 | 7.538 → 6.664 |
| GNU 最小 | 2.361 → 1.558 | 2.970 → 2.996 | 8.389 → 7.611 |
| GNU JSON | 2.822 → 1.833 | 3.610 → 3.641 | 10.143 → 9.194 |
| musl 最小 | 5.796 → 5.021 | 7.013 → 7.073 | 24.345 → 23.649 |
| musl JSON | 6.904 → 5.974 | 8.417 → 8.464 | 29.180 → 28.299 |

部分检查移到 join，stackmap 阶段略增；主要收益来自取消代码／类型阶段重复的完整身份查询。最小／JSON 程序分别有 7,229／8,689 个 site，每个至少少一次查询；重复 ODR record 还会省去自身的 owner 查询与 frame 验证。三目标改动前后的全部计数一致，所有 site 与 stackmap 仍完整对应。

## 空间与构建

默认程序 `.text` 增量为 Darwin 16 B、GNU 128 B、musl 64 B；统计版为 16／64／64 B。两个程序的默认和统计版完整文件大小均不变，所有非代码段大小相同，包含静态表与 stackmap；没有新增常驻 owner 索引。

六组候选构建的 root `.slib` fingerprint 与基线一致，编译子进程均为零；编译器与系统 Cone 无需重建。仅更新三个目标的四种已有 runtime 配置。单次完整 runtime 冷准备的默认配置耗时为 4.50／5.15／11.54 s，旧记录为 5.03／5.16／6.00 s；统计版为 4.41／9.65／11.65 s，旧记录为 4.72／4.98／5.92 s。这些跨批次单次准备数据没有控制宿主负载，不能作为本项编译加速或回退的证据；实际 Scoop 编译复用由相同 fingerprint 与零子进程确认。

原始结果保存在两机忽略的 `tmp/program-performance/startup-owner-join/`，runtime／程序构建复用 `tmp/program-performance/startup-stats/`。首轮临时机器码检查修正了 Mach-O 符号前导下划线的匹配，产品代码及计时样本未受影响。注册记录汇集、其余 stackmap 解析和调用入口 TLS 成本继续按性能主线处理。
