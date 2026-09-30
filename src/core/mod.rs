//! 도메인 계층. UI 와 IO 구현에 의존하지 않는다.

pub mod clock;
pub mod error;
pub mod format;
pub mod id;
pub mod model;
pub mod service;
pub mod storage;

pub use error::PhError;
pub use model::{NewPrompt, Prompt, PromptId, PromptPatch, Scope};
