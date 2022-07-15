use crate::parser::internals::erasedset::ErasedSet;
use crate::parser::internals::syntax_base::builtin::Root;
use crate::{Node, NodeValue};
use crate::parser::MarkdownIt;

#[derive(Debug)]
pub struct InlineRoot {
    pub content: String,
    pub mapping: Vec<(usize, usize)>,
}

// this token is supposed to be replaced by one or many actual tokens by inline rule
impl NodeValue for InlineRoot {}

pub fn add(md: &mut MarkdownIt) {
    md.ruler.add("builtin::inline_parser", rule)
        .after("builtin::block_parser")
        .before_all();
}

pub fn rule(node: &mut Node, md: &MarkdownIt) {
    fn walk_recursive(node: &mut Node, md: &MarkdownIt, env: &mut ErasedSet) {
        let mut idx = 0;
        while idx < node.children.len() {
            let child = &mut node.children[idx];
            if let Some(data) = child.cast_mut::<InlineRoot>() {
                let content = std::mem::take(&mut data.content);
                let mapping = std::mem::take(&mut data.mapping);

                let mut root = std::mem::take(child);
                root.children = Vec::new();
                root = md.inline.parse(content, mapping, root, md, env);

                let len = root.children.len();
                node.children.splice(idx..idx+1, root.children);
                idx += len;
            } else {
                walk_recursive(child, md, env);
                idx += 1;
            }
        }
    }

    let data = node.cast_mut::<Root>().expect("expecting root node to always be Root");
    let mut env = std::mem::take(&mut data.env);

    walk_recursive(node, md, &mut env);

    let data = node.cast_mut::<Root>().unwrap();
    data.env = env;
}