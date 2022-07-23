//! Entity and numeric character references
//!
//! `&#123;`, `&#xAF;`, `&quot;`
//!
//! <https://spec.commonmark.org/0.30/#entity-and-numeric-character-references>
use once_cell::sync::Lazy;
use regex::Regex;
use crate::{MarkdownIt, Node};
use crate::common::utils::{get_entity_from_str, is_valid_entity_code};
use crate::parser::inline::{InlineRule, InlineState, TextSpecial};

pub fn add(md: &mut MarkdownIt) {
    md.inline.add_rule::<EntityScanner>();
}

static DIGITAL_RE : Lazy<Regex> = Lazy::new(|| {
    Regex::new("(?i)^&#((?:x[a-f0-9]{1,6}|[0-9]{1,7}));").unwrap()
});

static NAMED_RE : Lazy<Regex> = Lazy::new(|| {
    Regex::new("(?i)^&([a-z][a-z0-9]{1,31});").unwrap()
});

#[doc(hidden)]
pub struct EntityScanner;
impl InlineRule for EntityScanner {
    const MARKER: char = '&';

    fn check(state: &mut InlineState) -> Option<usize> {
        let mut chars = state.src[state.pos..state.pos_max].chars();
        if chars.next().unwrap() != '&' { return None; }

        let capture = if let Some('#') = chars.next() {
            DIGITAL_RE.captures(&state.src[state.pos..])
        } else {
            NAMED_RE.captures(&state.src[state.pos..])
        };

        Some(capture?[0].len())
    }

    fn run(state: &mut InlineState) -> Option<usize> {
        let mut chars = state.src[state.pos..state.pos_max].chars();
        if chars.next().unwrap() != '&' { return None; }

        if let Some('#') = chars.next() {
            parse_digital_entity(state)
        } else {
            parse_named_entity(state)
        }
    }
}

fn parse_digital_entity(state: &mut InlineState) -> Option<usize> {
    let capture = DIGITAL_RE.captures(&state.src[state.pos..])?;
    let entity_len = capture[0].len();
    let entity = &capture[1];

    #[allow(clippy::from_str_radix_10)]
    let code = if entity.starts_with('x') || entity.starts_with('X') {
        u32::from_str_radix(&entity[1..], 16).unwrap()
    } else {
        u32::from_str_radix(entity, 10).unwrap()
    };

    let content_str = if is_valid_entity_code(code) {
        char::from_u32(code).unwrap().into()
    } else {
        '\u{FFFD}'.into()
    };

    let markup_str = capture[0].to_owned();

    let mut node = Node::new(TextSpecial {
        content: content_str,
        markup: markup_str,
        info: "entity",
    });
    node.srcmap = state.get_map(state.pos, state.pos + entity_len);
    state.node.children.push(node);
    Some(entity_len)
}

fn parse_named_entity(state: &mut InlineState) -> Option<usize> {
    let capture = NAMED_RE.captures(&state.src[state.pos..])?;
    let str = get_entity_from_str(&capture[0])?;
    let entity_len = capture[0].len();
    let markup_str = capture[0].to_owned();
    let content_str = (*str).to_owned();

    let mut node = Node::new(TextSpecial {
        content: content_str,
        markup: markup_str,
        info: "entity",
    });
    node.srcmap = state.get_map(state.pos, state.pos + entity_len);
    state.node.children.push(node);
    Some(entity_len)
}
