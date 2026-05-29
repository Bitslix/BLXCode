mod common;
mod detect;
mod indexers;
pub mod state;
mod static_index;
mod unit;

pub use static_index::{
    generated_section_from_architecture_index, lint_architecture_impl, rebuild_architecture_impl,
    STATIC_BEGIN, STATIC_END,
};
