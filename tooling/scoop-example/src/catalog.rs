use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema: u32,
    pub recommended: Vec<String>,
    pub groups: Vec<Group>,
    pub topics: Vec<Topic>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Group {
    pub id: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub recommended: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Topic {
    pub id: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub keywords: Vec<String>,
    pub document: String,
    pub requirements: Vec<String>,
    pub related: Vec<String>,
    pub files: Vec<File>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct File {
    pub path: String,
    pub source: String,
    pub language: String,
}

fn relative_path(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('\\')
        && Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn id_part(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}

impl Catalog {
    pub fn parse(text: &str) -> Result<Self, String> {
        let catalog: Self = toml::from_str(text).map_err(|e| format!("索引格式错误：{e}"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema != 1 {
            return Err(format!("不支持的索引 schema：{}", self.schema));
        }
        let mut names = HashSet::new();
        let mut register = |id: &str, aliases: &[String]| -> Result<(), String> {
            for name in std::iter::once(id).chain(aliases.iter().map(String::as_str)) {
                if name.trim().is_empty() || name.trim() != name || !names.insert(name.to_owned()) {
                    return Err(format!("重复或无效的 ID/别名：{name:?}"));
                }
            }
            Ok(())
        };
        for group in &self.groups {
            if !id_part(&group.id) || !self.topics.iter().any(|t| t.group() == group.id) {
                return Err(format!("无效或没有主题的分类：{}", group.id));
            }
            register(&group.id, &group.aliases)?;
        }
        for topic in &self.topics {
            let parts: Vec<_> = topic.id.split('/').collect();
            if parts.len() != 2
                || !parts.iter().all(|part| id_part(part))
                || !self.groups.iter().any(|g| g.id == parts[0])
            {
                return Err(format!("无效的主题 ID 或分类：{}", topic.id));
            }
            register(&topic.id, &topic.aliases)?;
            let mut paths = HashSet::new();
            if !relative_path(&topic.document) || topic.files.is_empty() {
                return Err(format!("主题缺少正文或文件：{}", topic.id));
            }
            for file in &topic.files {
                if !relative_path(&file.path)
                    || !relative_path(&file.source)
                    || !paths.insert(&file.path)
                {
                    return Err(format!("{}：无效或重复的文件路径 {}", topic.id, file.path));
                }
            }
        }
        for id in self.recommended.iter().chain(
            self.groups
                .iter()
                .flat_map(|g| &g.recommended)
                .chain(self.topics.iter().flat_map(|t| &t.related)),
        ) {
            if !self.topics.iter().any(|t| t.id == *id) {
                return Err(format!("引用了未发布主题：{id}"));
            }
        }
        for group in &self.groups {
            if group
                .recommended
                .iter()
                .any(|id| !id.starts_with(&format!("{}/", group.id)))
            {
                return Err(format!("分类推荐必须属于该分类：{}", group.id));
            }
        }
        Ok(())
    }

    pub fn topic(&self, name: &str) -> Option<&Topic> {
        self.topics
            .iter()
            .find(|t| t.id == name || t.aliases.iter().any(|alias| alias == name))
    }

    pub fn group(&self, name: &str) -> Option<&Group> {
        self.groups
            .iter()
            .find(|g| g.id == name || g.aliases.iter().any(|alias| alias == name))
    }
}

impl Topic {
    pub fn group(&self) -> &str {
        self.id.split('/').next().unwrap_or("")
    }
}
