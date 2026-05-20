//! Skills & Rules right-panel tabs.
//!
//! `state` is the shared service (`SkillsRulesService`) consumed by the two
//! tab docks and the install dialog.

pub mod install_dialog;
pub mod rule_card;
pub mod rules_tab;
pub mod skill_card;
pub mod skills_tab;
pub mod state;

pub use rules_tab::RulesTabDock;
pub use skills_tab::SkillsTabDock;
pub use state::SkillsRulesService;
