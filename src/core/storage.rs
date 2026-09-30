//! 저장소 추상화. 구현체는 `storage` 모듈에 있다.

use super::error::PhError;
use super::model::{Prompt, PromptId, Scope};

/// 한 scope 를 담당하는 저장소. 병합과 우선순위는 `PromptService` 가 처리한다.
pub trait Storage {
    /// 담당 scope.
    fn scope(&self) -> Scope;
    /// 표시용 위치 (fs: 디렉터리 경로, memory: "memory").
    fn location(&self) -> String;
    /// 전체 목록. 읽을 수 없는 항목은 실패시키지 않고 `skipped` 로 돌려준다.
    fn list(&self) -> Result<Listing, PhError>;
    /// 하나 조회. 없으면 `Ok(None)`.
    fn get(&self, id: &PromptId) -> Result<Option<Prompt>, PhError>;
    /// 생성 또는 덮어쓰기. atomic 해야 한다.
    fn put(&self, prompt: &Prompt) -> Result<(), PhError>;
    /// 삭제. 없으면 `NotFound`.
    fn delete(&self, id: &PromptId) -> Result<(), PhError>;
}

/// `Storage::list` 결과.
#[derive(Debug, Default)]
pub struct Listing {
    /// 읽은 prompt
    pub prompts: Vec<Prompt>,
    /// 읽지 못하고 건너뛴 항목
    pub skipped: Vec<SkippedEntry>,
}

/// 건너뛴 항목.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedEntry {
    /// 파일 이름 등 식별 정보
    pub name: String,
    /// 건너뛴 이유
    pub reason: String,
}
