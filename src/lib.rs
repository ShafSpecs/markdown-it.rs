#![forbid(unsafe_code)]
#![warn(clippy::manual_assert)]
#![warn(clippy::semicolon_if_nothing_returned)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_else_if)]

pub mod common;
pub mod generics;
pub mod parser;
pub mod plugins;
pub mod examples;

pub use parser::node::{Node, NodeValue};
pub use parser::main::MarkdownIt;
pub use parser::renderer::Renderer;
