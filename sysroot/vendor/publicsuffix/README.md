# Public Suffix List

固定快照 `2026-10-17_18-52-06_UTC`，revision `52b71b3c4576ff77450cc7ebcfc7f34a916af3f9`。原始 `public_suffix_list.dat` 来自 [PSL 官方数据地址](https://publicsuffix.org/list/public_suffix_list.dat)，保留 ICANN 和 PRIVATE 两个分区及原始维护说明。

数据遵循 MPL-2.0，完整许可保存在 LICENSE。`test_psl.txt` 来自相同 revision 的[官方测试](https://github.com/publicsuffix/list/blob/52b71b3c4576ff77450cc7ebcfc7f34a916af3f9/tests/test_psl.txt)，其 CC0 声明保留在文件开头。

维护者更新原始数据后，显式运行 `python3 scripts/generate_public_suffix.py`，并运行 HTTP 的 PublicSuffixTests 和正式消费者。生成的 `.inc` 只保留规则、分区和版本注释，由 HTTP Cone 的普通 C 静态数据引用；域名规范化及匹配仍由 Scoop 的 idna／HTTP 代码负责。普通构建和请求不执行更新脚本，也不下载数据。
