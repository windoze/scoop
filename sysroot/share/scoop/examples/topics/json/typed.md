## 运行

在保存文件的目录执行：

```sh
scoop run --release typed.scoop
```

## 预期结果

程序 stdout：

```text
{"name":"Ada","scores":[7,9]}
Ada: 9
no note
```

## 关键说明

User 的 public companion 同时声明 ValueEncoder<User> 和 ValueDecoder<User>，为这些库入口提供类型 codec。`Json.encodeToString(original)` 和 `Json.decodeFromString<User>(text)` 通过公开 companion 能力使用 codec。需要自己选择 codec 时，也可使用 API 的显式 encoder/decoder 参数入口。

note 的 String? 用 None 表示缺失值，当前默认编码结果省略这个字段；解码时使用声明中的默认值 None，往返后仍是 None。不能把这里的缺省字段行为推广为所有格式和所有编码入口的规则。scores 是 Array<Int>；索引是 Long。

可以修改 DTO 字段，但所用字段类型也必须满足 codec 要求；若希望缺字段时有默认结果，应在声明中提供实际默认值。

## 常见误用

不要仅声明 struct 就期待任意 JSON 入口都能自动找到 codec。这里依赖 public companion；internal companion 不能满足要求公开能力的泛型入口。不要把 JSON null 与 Scoop 语言中存在 null 值混为一谈。
