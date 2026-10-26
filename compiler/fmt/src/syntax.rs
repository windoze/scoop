use scoop_syntax::{SyntaxKind as K, SyntaxNode, SyntaxToken};

pub(crate) struct Token {
    pub syntax: SyntaxToken,
    pub before: String,
}

impl Token {
    pub fn kind(&self) -> K {
        self.syntax.kind()
    }

    pub fn parent_kind(&self) -> K {
        self.operator()
            .or_else(|| self.syntax.parent())
            .expect("source tokens have a parent")
            .kind()
    }

    pub fn operator(&self) -> Option<SyntaxNode> {
        self.syntax.parent_ancestors().find(|node| {
            matches!(
                node.kind(),
                K::BinaryOperator | K::PrefixOperator | K::PostfixOperator
            )
        })
    }

    pub fn newlines(&self) -> usize {
        self.before
            .replace("\r\n", "\n")
            .chars()
            .filter(|c| matches!(c, '\n' | '\r'))
            .count()
    }

    pub fn is_comment(&self) -> bool {
        self.kind().is_trivia()
    }
}

pub(crate) struct Input {
    pub tokens: Vec<Token>,
    pub nodes: Vec<(K, usize, usize, Option<K>)>,
    pub crlf: bool,
    pub expressions: Vec<(usize, usize)>,
}

impl Input {
    pub fn new(root: &SyntaxNode) -> Self {
        let mut tokens = Vec::new();
        let mut whitespace = String::new();
        let mut has_crlf = false;
        let mut has_other_newline = false;
        for token in root
            .descendants_with_tokens()
            .filter_map(|item| item.into_token())
        {
            if token.kind() == K::Whitespace {
                let text = token.text();
                has_crlf |= text.contains("\r\n");
                has_other_newline |= text.replace("\r\n", "").contains(['\r', '\n']);
                whitespace.push_str(text);
            } else if token.kind() != K::Eof {
                tokens.push(Token {
                    syntax: token,
                    before: std::mem::take(&mut whitespace),
                });
            }
        }
        let mut expressions = Vec::new();
        let nodes = root
            .descendants()
            .map(|node| {
                let range = node.text_range();
                let start = tokens
                    .partition_point(|token| token.syntax.text_range().end() <= range.start());
                let end =
                    tokens.partition_point(|token| token.syntax.text_range().start() < range.end());
                if node.kind() == K::Expression
                    && node
                        .children()
                        .any(|child| child.kind() == K::BinaryOperator)
                {
                    expressions.push((start, end));
                }
                (
                    node.kind(),
                    start,
                    end,
                    node.parent().map(|parent| parent.kind()),
                )
            })
            .filter(|(_, start, end, _)| start < end)
            .collect();
        Self {
            tokens,
            nodes,
            crlf: has_crlf && !has_other_newline,
            expressions,
        }
    }
}
