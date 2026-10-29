//! pm-ai: AI-PM instance model, skills, policy engine, executor.

pub mod fake_model;
pub mod model;
pub mod skill_router;

pub use fake_model::FakeModel;
pub use model::{AnalysisContext, AnalysisModel, ModelError};
pub use skill_router::{route, SkillId};
