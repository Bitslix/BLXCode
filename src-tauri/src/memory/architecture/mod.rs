pub mod state;
mod static_index;

pub use static_index::{
    generated_section_from_architecture_index, lint_architecture_impl, rebuild_architecture_impl,
    STATIC_BEGIN, STATIC_END,
};
