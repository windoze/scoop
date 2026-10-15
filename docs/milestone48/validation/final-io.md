# M48 最终外部调用与 I/O 观察

功能验收结束后，在 nuc12 上用本批 release 工具和 debug Scoop profile 执行实际构建；源码、工具与原产物全程保留。GNU 五个场景各五组冷／暖，native 另五次头文件更新，共 55 次；musl 补验 empty、native 两个代表场景，共 5 次。所有程序均运行并核对输出。

`strace -ff` 捕获真实 `execve`；暖构建另记录 open 和 rename，依赖读取单独记录 read／pread／lseek。原始数据保存在仓库外。统计按实际 executable 路径及 argv 分类，预处理不计入 native 编译，linker 的配置查询、能力探测链接和最终程序链接分别计数。trace 耗时不参与[Darwin 性能对照](../PERFORMANCE.md)。

## GNU 调用计数

下表每行五次观察的计数完全一致，故中位数、最小值和最大值相同。native 编译包括真实 C 工具能力探测，不能将其直接解释为生产源码文件数。

| 场景／阶段 | 成功 execve（含 scoop） | Scoop 编译 child | native 编译 | native 预处理 | linker 配置查询 | 探测链接 | 最终链接 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| core/cold | 13 | 1 | 1 | 2 | 0 | 0 | 0 |
| std/cold | 14 | 2 | 1 | 2 | 0 | 0 | 0 |
| empty/cold | 501 | 2 | 97 | 96 | 2 | 1 | 1 |
| application/cold | 505 | 6 | 97 | 96 | 2 | 1 | 1 |
| native/cold | 516 | 2 | 100 | 99 | 2 | 1 | 1 |
| native/header-change | 17 | 1 | 3 | 3 | 0 | 0 | 0 |
| 五个场景各自的 warm | 1 | 0 | 0 | 0 | 0 | 0 | 0 |

native 更新测量在真实头文件末尾添加注释，触发一个 Cone 的正常更新判断和 native 编译；编译结果的语义未变，最终链接仍复用。头文件内容改变运行结果、include 选择改变及 runtime 更新另由正式 CLI 功能用例覆盖。

## musl 代表观察

empty/cold 为 703 次 execve，native/cold 为 724 次，native/header-change 为 23 次；对应 Scoop 编译、native 编译／预处理、linker 查询／探测／最终链接计数与 GNU 相同。musl 工具包装器还会调用实际 GCC，额外的 wrapper exec 单列于原始统计，不重复计为一次 native 编译。两个暖构建均只有 scoop 自身一次 execve，未发生编译或链接。

## 暖构建文件操作

GNU 25 次和 musl 2 次暖构建均只有一次 execve 尝试，无失败的额外工具启动；未打开 Scoop／C／C++ 源码、头文件、Cone.toml、`.slib`、对象、归档或受检工具／程序内容，rename 为零。未出现锁文件以外的创建或截断打开；普通构建锁及小型更新记录照常使用。程序自身装载所需的系统动态库不属于编译输入内容读取。

## 下游按需读取

使用当前 `scoopc` 对一个真实下游库执行源码编译，直接消费本轮 core `.slib`，覆盖 HIR／MIR／LIR 消费路径。39,760,536 字节的依赖只打开一次；读取 manifest 和三个 metadata member，共读取 27,646,123 字节，读取区间没有重叠。4,207 个机器对象 member 均未读取。该观察直接验证实际文件 reader，不以进程采样或测试替身推断 I/O。
