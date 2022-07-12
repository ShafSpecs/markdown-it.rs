// Process '\n'
//
use crate::{Node, NodeValue, Renderer, HtmlElement};
use crate::parser::MarkdownIt;
use crate::parser::internals::inline;

#[derive(Debug)]
pub struct Hardbreak;

impl NodeValue for Hardbreak {
    fn render(&self, _: &Node, fmt: &mut dyn Renderer) {
        fmt.self_close("br", &[]);
        fmt.cr();
    }

    fn render2(&self, node: &Node) -> crate::Html {
        crate::Html::Element(HtmlElement {
            tag: "br",
            attrs: vec![],
            children: None,
            spacing: crate::HtmlSpacing::After,
        })
    }
}

#[derive(Debug)]
pub struct Softbreak;

impl NodeValue for Softbreak {
    fn render(&self, _: &Node, fmt: &mut dyn Renderer) {
        fmt.cr();
    }

    fn render2(&self, node: &Node) -> crate::Html {
        crate::Html::RawText("\n".to_owned())
    }
}

pub fn add(md: &mut MarkdownIt) {
    md.inline.ruler.add("newline", rule);
}

fn rule(state: &mut inline::State, silent: bool) -> bool {
    let mut chars = state.src[state.pos..state.pos_max].chars();

    if chars.next().unwrap() != '\n' { return false; }

    let mut pos = state.pos;
    pos += 1;

    // skip leading whitespaces from next line
    while let Some(' ' | '\t') = chars.next() {
        pos += 1;
    }

    // '  \n' -> hardbreak
    if !silent {
        let mut tail_size = 0;
        let trailing_text = state.trailing_text_get();

        for ch in trailing_text.chars().rev() {
            if ch == ' ' {
                tail_size += 1;
            } else {
                break;
            }
        }

        state.trailing_text_pop(tail_size);

        let mut node = if tail_size >= 2 {
            Node::new(Hardbreak)
        } else {
            Node::new(Softbreak)
        };

        node.srcmap = state.get_map(state.pos - tail_size, pos);
        state.push(node);
    }

    state.pos = pos;
    true
}
