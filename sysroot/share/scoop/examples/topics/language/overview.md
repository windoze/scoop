## 运行

在保存文件的目录执行：

```sh
scoop run --release values.scoop
```

## 预期结果

程序 stdout：

```text
point=(3, 4)
point missing
Ada
guest
3
```

## 关键说明

Point 是真正的值类型；Lookup 是带载荷的 enum。`when` 中的 `case` 解构载荷并覆盖每个变体。格式化字符串显式使用 `f"...${expression}..."`。

`String?` 是 `Option<String>`，用 Some/None 表示有值或缺失。`?:` 给出缺失时的结果，`?.` 仅在有值时读取属性。索引和长度等 API 使用 Long，因此默认长度写 `0L`。这是一页语法概览，具体约束由相关主题解释。

## 常见误用

不要按 Kotlin 的 null、data class 或 enum class 语义推断这里的写法。普通字符串不会插值；值类型的接口能力也不能凭名称猜测。
