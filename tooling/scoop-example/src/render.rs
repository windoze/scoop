use crate::catalog::{Catalog, Topic};
use crate::{Args, COMMAND_NAME, Error, Format, read, search};
use serde_json::{Value, json};
use std::path::Path;

fn summary(topic: &Topic) -> Value {
    json!({"id": topic.id, "title": topic.title, "summary": topic.summary})
}

fn entries(catalog: &Catalog, ids: &[String]) -> Vec<Value> {
    ids.iter()
        .filter_map(|id| catalog.topic(id))
        .map(summary)
        .collect()
}

fn entry_markdown(topic: &Topic) -> String {
    format!("- `{}` — {}：{}\n", topic.id, topic.title, topic.summary)
}

fn append_recommendations(text: &mut String, catalog: &Catalog, ids: &[String]) {
    if !ids.is_empty() {
        text.push_str("\n## 建议起点\n\n");
        for topic in ids.iter().filter_map(|id| catalog.topic(id)) {
            text.push_str(&entry_markdown(topic));
        }
    }
}

fn version(root: &Path) -> Result<String, Error> {
    match std::fs::read_to_string(root.join("VERSION")) {
        Ok(text) => Ok(text.trim().to_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            Ok("开发内容，未提供发行版本".to_owned())
        }
        Err(e) => Err(Error::content(format!("无法读取 VERSION：{e}"))),
    }
}

fn finish(format: Format, value: Value, markdown: String) -> Result<String, Error> {
    if format == Format::Json {
        serde_json::to_string_pretty(&value)
            .map(|text| text + "\n")
            .map_err(|e| Error::content(e.to_string()))
    } else {
        Ok(markdown)
    }
}

fn topic_output(
    root: &Path,
    directory: &Path,
    catalog: &Catalog,
    topic: &Topic,
    args: &Args,
) -> Result<String, Error> {
    if args.format == Format::Source {
        let file = match args.file.as_deref() {
            Some(path) => topic.files.iter().find(|f| f.path == path),
            None if topic.files.len() == 1 => topic.files.first(),
            None => None,
        };
        let file = file.ok_or_else(|| {
            Error::usage(format!(
                "请用 --file 选择 {} 的文件：{}",
                topic.id,
                topic
                    .files
                    .iter()
                    .map(|f| f.path.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
        return read(&root.join(&file.source));
    }
    let version = version(root)?;
    let body = read(&directory.join(&topic.document))?;
    let mut markdown = format!(
        "# {} — {}\n\n工具链：{}\n\n{}\n\n## 运行条件\n\n",
        topic.id,
        topic.title,
        version.replace('\n', " · "),
        topic.summary
    );
    for requirement in &topic.requirements {
        markdown.push_str(&format!("- {requirement}\n"));
    }
    markdown.push_str("\n将以下完整文件按显示路径保存在可写目录中。查询选用其他安装根时，后续命令也使用相同的 `scoop --sysroot <路径>`。\n\n## 完整文件\n");
    let mut files = Vec::new();
    for file in &topic.files {
        let text = read(&root.join(&file.source))?;
        let fence = "`".repeat(
            text.split(|c| c != '`')
                .map(str::len)
                .max()
                .unwrap_or(0)
                .max(2)
                + 1,
        );
        markdown.push_str(&format!(
            "\n### {}\n\n来源：`{}`\n\n{fence}{}\n{}{}{fence}\n",
            file.path,
            file.source,
            file.language,
            text,
            if text.ends_with('\n') { "" } else { "\n" }
        ));
        files.push(json!({"path": file.path, "source": file.source, "language": file.language, "text": text}));
    }
    markdown.push('\n');
    markdown.push_str(&body);
    if !body.ends_with('\n') {
        markdown.push('\n');
    }
    markdown.push_str("\n## 相关主题\n\n");
    for related in topic.related.iter().filter_map(|id| catalog.topic(id)) {
        markdown.push_str(&entry_markdown(related));
    }
    finish(
        args.format,
        json!({
            "schema": 1, "kind": "topic", "version": version,
            "id": topic.id, "title": topic.title, "summary": topic.summary,
            "requirements": topic.requirements, "files": files,
            "body_markdown": body, "related": entries(catalog, &topic.related)
        }),
        markdown,
    )
}

pub fn render(
    root: &Path,
    directory: &Path,
    catalog: &Catalog,
    args: &Args,
) -> Result<String, Error> {
    if let Some(name) = &args.topic {
        if let Some(topic) = catalog.topic(name) {
            return topic_output(root, directory, catalog, topic, args);
        }
        if let Some(group) = catalog.group(name) {
            if args.format == Format::Source {
                return Err(Error::usage("分类没有单一源码；请指定一个完整主题。"));
            }
            let topics: Vec<_> = catalog
                .topics
                .iter()
                .filter(|t| t.group() == group.id)
                .collect();
            let mut markdown = format!("# {} — {}\n\n{}\n", group.id, group.title, group.summary);
            append_recommendations(&mut markdown, catalog, &group.recommended);
            markdown.push_str("\n## 主题\n\n");
            for topic in &topics {
                markdown.push_str(&entry_markdown(topic));
            }
            return finish(
                args.format,
                json!({
                    "schema": 1, "kind": "group", "version": version(root)?,
                    "id": group.id, "title": group.title, "summary": group.summary,
                    "recommended": entries(catalog, &group.recommended),
                    "topics": topics.iter().map(|t| summary(t)).collect::<Vec<_>>()
                }),
                markdown,
            );
        }
        let candidates = search::search(catalog, name);
        let hints = candidates
            .iter()
            .take(5)
            .map(|h| h.topic.id.as_str())
            .collect::<Vec<_>>();
        return Err(Error::content(format!(
            "未知分类或主题：{name}。使用 {COMMAND_NAME} --list 或 --search 查询。{}",
            if hints.is_empty() {
                String::new()
            } else {
                format!("候选：{}", hints.join(", "))
            }
        )));
    }
    if let Some(query) = &args.search {
        let hits = search::search(catalog, query);
        if hits.is_empty() {
            return Err(Error::content(format!(
                "没有匹配 {query:?} 的主题；使用 --list 查看目录。"
            )));
        }
        let mut markdown = format!("# 搜索：{query}\n\n");
        let mut results = Vec::new();
        for hit in hits {
            markdown.push_str(&entry_markdown(hit.topic));
            markdown.push_str(&format!("  匹配：{}\n", hit.matched_fields.join(", ")));
            let mut item = summary(hit.topic);
            item["matched_fields"] = json!(hit.matched_fields);
            results.push(item);
        }
        return finish(
            args.format,
            json!({
                "schema": 1, "kind": "search", "version": version(root)?, "query": query, "topics": results
            }),
            markdown,
        );
    }
    let mut markdown = format!(
        "# Scoop 示例\n\n使用 `{COMMAND_NAME} <分类/主题>` 阅读，或用 `--search \"关键词\"` 搜索。\n"
    );
    if !args.list {
        append_recommendations(&mut markdown, catalog, &catalog.recommended);
    }
    for group in &catalog.groups {
        markdown.push_str(&format!(
            "\n## {} — {}\n\n{}\n",
            group.id, group.title, group.summary
        ));
        if args.list {
            markdown.push('\n');
            for topic in catalog.topics.iter().filter(|t| t.group() == group.id) {
                markdown.push_str(&entry_markdown(topic));
            }
        }
    }
    let mut value = json!({
        "schema": 1, "kind": if args.list { "list" } else { "index" },
        "version": version(root)?, "groups": catalog.groups,
        "recommended": entries(catalog, &catalog.recommended)
    });
    if args.list {
        value["topics"] = json!(catalog.topics.iter().map(summary).collect::<Vec<_>>());
    }
    finish(args.format, value, markdown)
}
