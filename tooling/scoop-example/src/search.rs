use crate::catalog::{Catalog, Topic};

pub struct Hit<'a> {
    pub topic: &'a Topic,
    pub matched_fields: Vec<&'static str>,
}

pub fn search<'a>(catalog: &'a Catalog, query: &str) -> Vec<Hit<'a>> {
    let query = query.trim().to_lowercase();
    let words: Vec<_> = query.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for topic in &catalog.topics {
        let fields = [
            ("id", topic.id.to_lowercase()),
            ("title", topic.title.to_lowercase()),
            ("aliases", topic.aliases.join(" ").to_lowercase()),
            ("keywords", topic.keywords.join(" ").to_lowercase()),
            ("summary", topic.summary.to_lowercase()),
        ];
        if !words
            .iter()
            .all(|word| fields.iter().any(|(_, text)| text.contains(word)))
        {
            continue;
        }
        let rank = if topic.id.to_lowercase() == query
            || topic.aliases.iter().any(|a| a.to_lowercase() == query)
        {
            0
        } else if topic.id.to_lowercase().starts_with(&query) {
            1
        } else if words
            .iter()
            .all(|word| fields[..4].iter().any(|(_, t)| t.contains(word)))
        {
            2
        } else {
            3
        };
        hits.push((
            rank,
            Hit {
                topic,
                matched_fields: fields
                    .iter()
                    .filter(|(_, text)| words.iter().any(|word| text.contains(word)))
                    .map(|(name, _)| *name)
                    .collect(),
            },
        ));
    }
    // Stable sorting retains the catalog's editorial order for equally useful matches.
    hits.sort_by_key(|(rank, _)| *rank);
    hits.into_iter().map(|(_, hit)| hit).collect()
}
