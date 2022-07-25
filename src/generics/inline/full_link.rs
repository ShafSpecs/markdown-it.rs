//! Structure similar to `[link](<to> "stuff")` with configurable prefix.
//!
//! There are two structures in CommonMark that match this syntax:
//!  - links - `[text](<href> "title")`
//!  - images - `![alt](<src> "title")`
//!
//! You can add custom rules like `~[foo](<bar> "baz")`. Let us know if
//! you come up with fun use case to add as an example!
//!
//! Add a custom structure by using [add_prefix] function, which takes following arguments:
//!  - `PREFIX` - marker character before label (`!` in case of images)
//!  - `ENABLE_NESTED` - allow nested links inside
//!  - `md` - parser instance
//!  - `f` - function that should return your custom [Node] given href and title
//!
use std::collections::HashMap;
use crate::{MarkdownIt, Node};
use crate::common::utils::unescape_all;
use crate::parser::inline::{InlineRule, InlineState};
use crate::plugins::cmark::block::reference::{ReferenceMap, ReferenceMapKey};

#[derive(Debug)]
struct LinkCfg<const PREFIX: char>(fn (Option<String>, Option<String>) -> Node);

/// adds custom rule with no prefix
pub fn add<const ENABLE_NESTED: bool>(
    md: &mut MarkdownIt,
    f: fn (url: Option<String>, title: Option<String>) -> Node
) {
    md.env.insert(LinkCfg::<'\0'>(f));
    md.inline.add_rule::<LinkScanner<ENABLE_NESTED>>();
    if !md.inline.has_rule::<LinkScannerEnd>() {
        md.inline.add_rule::<LinkScannerEnd>();
    }
}

/// adds custom rule with given `PREFIX` character
pub fn add_prefix<const PREFIX: char, const ENABLE_NESTED: bool>(
    md: &mut MarkdownIt,
    f: fn (url: Option<String>, title: Option<String>) -> Node
) {
    md.env.insert(LinkCfg::<PREFIX>(f));
    md.inline.add_rule::<LinkPrefixScanner<PREFIX, ENABLE_NESTED>>();
    if !md.inline.has_rule::<LinkScannerEnd>() {
        md.inline.add_rule::<LinkScannerEnd>();
    }
}

#[doc(hidden)]
pub struct LinkScanner<const ENABLE_NESTED: bool>;
impl<const ENABLE_NESTED: bool> InlineRule for LinkScanner<ENABLE_NESTED> {
    const MARKER: char = '[';

    fn run(state: &mut InlineState) -> Option<usize> {
        let mut chars = state.src[state.pos..state.pos_max].chars();
        if chars.next().unwrap() != '[' { return None; }
        let f = state.md.env.get::<LinkCfg<'\0'>>().unwrap().0;
        rule(state, ENABLE_NESTED, 0, f)
    }
}

#[doc(hidden)]
pub struct LinkPrefixScanner<const PREFIX: char, const ENABLE_NESTED: bool>;
impl<const PREFIX: char, const ENABLE_NESTED: bool> InlineRule for LinkPrefixScanner<PREFIX, ENABLE_NESTED> {
    const MARKER: char = PREFIX;

    fn run(state: &mut InlineState) -> Option<usize> {
        let mut chars = state.src[state.pos..state.pos_max].chars();
        if chars.next() != Some(PREFIX) { return None; }
        if chars.next() != Some('[') { return None; }
        let f = state.md.env.get::<LinkCfg<PREFIX>>().unwrap().0;
        rule(state, ENABLE_NESTED, 1, f)
    }
}

#[doc(hidden)]
pub struct LinkScannerEnd;
impl InlineRule for LinkScannerEnd {
    const MARKER: char = ']';

    fn run(_: &mut InlineState) -> Option<usize> {
        None
    }
}

#[derive(Debug)]
struct SeenLinks;

fn rule(
    state: &mut InlineState,
    enable_nested: bool,
    offset: usize,
    f: fn (Option<String>, Option<String>) -> Node
) -> Option<usize> {
    // possibility of recursion via either skip_token or tokenize
    stacker::maybe_grow(64*1024, 1024*1024, || {
        let start = state.pos;

        if let Some(result) = parse_link(state, state.pos + offset, enable_nested) {
            //
            // We found the end of the link, and know for a fact it's a valid link;
            // so all that's left to do is to call tokenizer.
            //
            let mut node = f(result.href, result.title);
            node.srcmap = state.get_map(start, result.end);
            node.children = result.nodes;
            state.node.children.push(node);
            if !enable_nested { state.inline_env.insert(SeenLinks); }
            Some(result.end - state.pos)
        } else {
            None
        }
    })
}

#[derive(Debug, Default)]
struct LinkLabelScanCache(HashMap<usize, Option<(Vec<Node>, usize)>>);

// Parse link label
//
// this function assumes that first character ("[") already matches;
// returns the end of the label
//
// fills cache as the result
fn parse_link_label(state: &mut InlineState, start: usize, enable_nested: bool) -> Option<usize> {
    let cache = state.inline_env.get_or_insert_default::<LinkLabelScanCache>();
    if let Some(result) = cache.0.get_mut(&start) {
        return if let Some((_, label_end)) = *result {
            Some(label_end)
        } else {
            None
        }
    }

    let oldroot = std::mem::take(&mut state.node);
    let oldseenlinks = state.inline_env.remove::<SeenLinks>();
    let old_pos = state.pos;
    let mut found = false;
    let mut level = 1;

    state.pos = start + 1;

    while let Some(ch) = state.src[state.pos..state.pos_max].chars().next() {
        if ch == ']' {
            level -= 1;
            if level == 0 {
                found = true;
                break;
            }
        }

        let found_nontext_token = state.md.inline.tokenize_one(state).unwrap_or_default();

        if !enable_nested && state.inline_env.contains::<SeenLinks>() {
            break;
        }

        if !found_nontext_token && ch == '[' {
            // increase level if we find text `[`, which is not a part of any token
            level += 1;
        }
    }

    let cache = state.inline_env.get_or_insert_default::<LinkLabelScanCache>();
    let result = if found {
        cache.0.insert(start, Some((std::mem::take(&mut state.node.children), state.pos)));
        Some(state.pos)
    } else {
        // [ [ [ [ [... case
        //         ^ if we didn't find a closer here,
        //       ^ these won't find it either
        cache.0.insert(start, None);
        None
    };

    // restore old state
    state.pos = old_pos;
    state.node = oldroot;
    if oldseenlinks.is_some() { state.inline_env.insert(SeenLinks); }

    result
}


pub struct ParseLinkFragmentResult {
    /// end position
    pub pos:   usize,
    /// number of linebreaks inside
    pub lines: usize,
    /// parsed result
    pub str:   String,
}


/// Helper function used to parse `<href>` part of the links with optional brackets.
pub fn parse_link_destination(str: &str, start: usize, max: usize) -> Option<ParseLinkFragmentResult> {
    let mut chars = str[start..max].chars().peekable();
    let mut pos = start;

    if let Some('<') = chars.peek() {
        chars.next(); // skip '<'
        pos += 1;
        loop {
            match chars.next() {
                Some('\n' | '<') | None => return None,
                Some('>') => {
                    return Some(ParseLinkFragmentResult {
                        pos: pos + 1,
                        lines: 0,
                        str: unescape_all(&str[start + 1..pos]).into_owned(),
                    });
                }
                Some('\\') => {
                    match chars.next() {
                        None => return None,
                        Some(x) => pos += 1 + x.len_utf8(),
                    }
                }
                Some(x) => {
                    pos += x.len_utf8();
                }
            }
        }
    } else {
        let mut level : u32 = 0;
        loop {
            match chars.next() {
                // space + ascii control characters
                Some('\0'..=' ' | '\x7f') | None => break,
                Some('\\') => {
                    match chars.next() {
                        Some(' ') | None => break,
                        Some(x) => pos += 1 + x.len_utf8(),
                    }
                }
                Some('(') => {
                    level += 1;
                    if level > 32 { return None; }
                    pos += 1;
                }
                Some(')') => {
                    if level == 0 { break; }
                    level -= 1;
                    pos += 1;
                }
                Some(x) => {
                    pos += x.len_utf8();
                }
            }
        }

        if level != 0 { return None; }

        Some(ParseLinkFragmentResult {
            pos,
            lines: 0,
            str: unescape_all(&str[start..pos]).into_owned(),
        })
    }
}


/// Helper function used to parse `"title"` part of the links (with `'title'` or `(title)` alternative syntax).
pub fn parse_link_title(str: &str, start: usize, max: usize) -> Option<ParseLinkFragmentResult> {
    let mut chars = str[start..max].chars();
    let mut pos = start + 1;
    let mut lines = 0;

    let marker = match chars.next() {
        Some('"')  => '"',
        Some('\'') => '\'',
        Some('(')  => ')',
        None | Some(_) => return None,
    };

    loop {
        match chars.next() {
            Some(ch) if ch == marker => {
                return Some(ParseLinkFragmentResult {
                    pos: pos + 1,
                    lines,
                    str: unescape_all(&str[start + 1..pos]).into_owned(),
                });
            }
            Some('(') if marker == ')' => {
                return None;
            }
            Some('\n') => {
                pos += 1;
                lines += 1;
            }
            Some('\\') => {
                match chars.next() {
                    None => return None,
                    Some(x) => pos += 1 + x.len_utf8(),
                }
            }
            Some(x) => {
                pos += x.len_utf8();
            }
            None => {
                return None;
            }
        }
    }
}

struct ParseLinkResult {
    pub nodes: Vec<Node>,
    pub href: Option<String>,
    pub title: Option<String>,
    pub end: usize,
}

// Parses [link](<to> "stuff")
//
// this function assumes that first character ("[") already matches
//
fn parse_link(state: &mut InlineState, pos: usize, enable_nested: bool) -> Option<ParseLinkResult> {
    let label_end = parse_link_label(state, pos, enable_nested)?;
    let label_start = pos + 1;
    let mut pos = label_end + 1;
    let mut chars = state.src[pos..state.pos_max].chars();
    let mut href = None;
    let mut title = None;

    if let Some('(') = chars.next() {
        //
        // Inline link
        //

        // [link](  <href>  "title"  )
        //        ^^ skipping these spaces
        pos += 1;
        while let Some(' ' | '\t' | '\n') = chars.next() {
            pos += 1;
        }

        // [link](  <href>  "title"  )
        //          ^^^^^^ parsing link destination
        if let Some(res) = parse_link_destination(&state.src, pos, state.pos_max) {
            let href_candidate = (state.md.normalize_link)(&res.str);
            if (state.md.validate_link)(&href_candidate) {
                pos = res.pos;
                href = Some(href_candidate);
            }

            // [link](  <href>  "title"  )
            //                ^^ skipping these spaces
            let mut chars = state.src[pos..state.pos_max].chars();
            while let Some(' ' | '\t' | '\n') = chars.next() {
                pos += 1;
            }

            if let Some(res) = parse_link_title(&state.src, pos, state.pos_max) {
                title = Some(res.str);
                pos = res.pos;

                // [link](  <href>  "title"  )
                //                         ^^ skipping these spaces
                let mut chars = state.src[pos..state.pos_max].chars();
                while let Some(' ' | '\t' | '\n') = chars.next() {
                    pos += 1;
                }
            }
        }

        if let Some(')') = state.src[pos..state.pos_max].chars().next() {
            let cache = state.inline_env.get_mut::<LinkLabelScanCache>().unwrap();
            let result = cache.0.remove(&(label_start - 1)).unwrap();

            return Some(ParseLinkResult {
                nodes: result.unwrap().0,
                href,
                title,
                end: pos + 1,
            })
        }
    }

    //
    // Link reference
    //
    // TODO: check if I have any references?
    pos = label_end + 1;
    let mut maybe_label = None;

    match state.src[pos..state.pos_max].chars().next() {
        Some('[') => {
            if let Some(x) = parse_link_label(state, pos, false) {
                maybe_label = Some(&state.src[pos + 1..x]);
                pos = x + 1;
            } else {
                pos = label_end + 1;
            }
        }
        _ => pos = label_end + 1,
    }

    if let Some(references) = state.root_env.get::<ReferenceMap>() {
        // covers label === '' and label === undefined
        // (collapsed reference link and shortcut reference link respectively)
        let label = if matches!(maybe_label, None | Some("")) {
            &state.src[label_start..label_end]
        } else {
            maybe_label.unwrap()
        };

        let lref = references.get(&ReferenceMapKey::new(label.to_owned()));
        let cache = state.inline_env.get_mut::<LinkLabelScanCache>().unwrap();
        let result = cache.0.remove(&(label_start - 1)).unwrap();

        lref.map(|r| ParseLinkResult {
            nodes: result.unwrap().0,
            href: Some(r.destination.clone()),
            title: r.title.clone(),
            end: pos,
        })
    } else {
        None
    }
}
