# M42 日期与时间

calendar 的 72 组固定位向量由 Python datetime 独立计算。超出 datetime
年份范围时将年份平移整数个 400 年至 2000..2399，再以每周期 146097 日
恢复 epoch day；星期与年内日序不变。覆盖负极值、year 0、世纪、闰年及正极值。
正式 fixture 只执行固定的 Scoop 向量，不调用宿主日期 API。

date-arithmetic 验证月份夹到月末、合并年月后一次应用、负年进位与完整域边界；
date-text 验证严格 ISO 年份、ASCII 分量与规范输出。集合组合验证日期的值相等、
Hash 与 B-tree 自然顺序。各运行 fixture 同时覆盖普通和 moving GC。

local-time 和 offset 独立检查整天回绕、Duration 极值余数、分量范围及严格文本；
date-time 检查 period 应用顺序、跨午夜、显式 offset 转换与域外失败。instant
同时执行 debug/release，区分负 epoch 毫秒的向下取整与 Duration 的向零取整，
检查全域差值、负极值调整及可回拨的假 Clock。time-contracts 锁定六条静态诊断。
artifacts 在移除 core/std/consumer 源码后独立链接，组合 seeded RNG、Uuid、
B-tree、Sequence 与日期时间转换，不依赖实际取时或系统时区。
