# 多线程推进与分配观测

复用 poll-response／parallel-allocation 的真实 Scoop callback、pthread 启停和普通
managed 循环方式。poll 模式由独立 collector 每轮 full 后等待 100 微秒；allocation
模式由 worker 分配实际 Item 对象，使用正常 nursery 自动收集，不增加独立 collector。
两种模式均在 1／4 个 mutator 下对照固定基线与当前版本，保留每线程 checksum。

每完成 1024 次实际计算／分配，Scoop 调用一次 GCLeaf 观测函数；首尾也发布进度。
collector 在 T1 保存每个仍工作线程最后一次已发布的时间和迭代数，在 T2 发布该轮记录。
worker 下一次发布进度时回填时间和迭代数，必须确实推进。若连续多轮 GC 之间没有发布
进度，多轮指向同一个后续观测点，原始记录保留此事实，不伪造逐轮 resume 时间。

last→first 是实际观测到的推进间隔，包含停稳前／恢复后的最多一个采样批次、GC、
解锁／广播和 OS 调度；T2→first 是允许恢复后到下一次推进观测的延迟。它们是有
采样粒度的实测间隔，不等于精确 park 时长。两侧执行同样的观测调用；测量只改 runtime
副本中的测试配置，不给生产分配、屏障或 poll 增加计时。旧／新 runtime 的 nursery
retry 观测均启用，记录 refill 重试数量，但不把该数量直接解释成锁等待时长。

先使用 build-progress.py 构建两套固定工具／runtime／sysroot 对应的 binary，完成
功能核对后再执行 measure-progress.py；计时期间不得并行构建、测试或清理目录。
报告保留 T0／T1／T2、逐轮逐线程进度、分配与收集指标、墙钟／CPU、RSS、吞吐及
命令和文件摘要。至少七次交替进程；分布尾部只描述该样本，不承诺稳定尾延迟。

```sh
export TMPDIR="$PWD/tmp/m44"
python3 tests/benchmarks/m44/build-progress.py \
  --tools target/m44/tools-operations --runtime runtime --sysroot sysroot \
  --work target/m44/progress-final-darwin --cache target/m44/codegen-benchmark-cache \
  --target aarch64-apple-darwin
python3 tests/benchmarks/m44/measure-progress.py \
  --binary baseline=target/m44/progress-base-final-darwin/progress \
  --binary candidate=target/m44/progress-final-darwin/progress \
  --output target/m44/progress-comparison-darwin.json --runs 7 --workers 4
```

基线按相同构建命令传入其固定工具、runtime 和 sysroot，另选未存在的 work 目录。
计算模式每线程一亿次迭代，分配模式每线程两百万个 24-byte Item；分配字节与所有
checksum 均核对。`test_progress.py` 检查缺失、顺序错误和只有时钟变化而无实际推进
的记录必须失败，连续 GC 可以共同等待同一个后续进度点。
