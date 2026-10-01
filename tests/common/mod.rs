//! 통합 테스트 공용 헬퍼. 실제 홈 디렉터리와 시스템 시각에 의존하지 않는다.
#![allow(dead_code)]

use chrono::{DateTime, FixedOffset};
use ph::core::clock::FixedClock;
use ph::core::model::{NewPrompt, Prompt, PromptId, Scope};
use ph::core::service::PromptService;
use ph::core::storage::Storage;
use ph::storage::{FsStorage, MemoryStorage};

pub fn t0() -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339("2026-09-30T12:00:00+09:00").unwrap()
}

pub fn prompt(id: &str, scope: Scope) -> Prompt {
    Prompt {
        id: PromptId::parse(id).unwrap(),
        scope,
        title: id.to_string(),
        body: format!("{id} 본문\n"),
        tags: vec!["t1".into()],
        description: Some("설명".into()),
        created_at: t0(),
        updated_at: t0(),
    }
}

pub fn new_prompt(title: &str) -> NewPrompt {
    NewPrompt {
        title: title.into(),
        body: format!("{title} 본문"),
        ..Default::default()
    }
}

/// 두 scope 모두 memory 인 서비스.
pub fn memory_service(with_local: bool) -> PromptService {
    PromptService::new(
        Box::new(MemoryStorage::new(Scope::Global)),
        with_local.then(|| Box::new(MemoryStorage::new(Scope::Local)) as Box<dyn Storage>),
        Box::new(FixedClock(t0())),
    )
}

/// 임시 디렉터리 기반 fs 서비스. (tempdir, global 디렉터리, local 디렉터리)
pub fn fs_service(
    with_local: bool,
) -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    PromptService,
) {
    let d = tempfile::tempdir().unwrap();
    let g = d.path().join("global").join("prompts");
    let l = d.path().join("proj").join(".ph").join("prompts");
    let svc = PromptService::new(
        Box::new(FsStorage::new(Scope::Global, g.clone())),
        with_local.then(|| Box::new(FsStorage::new(Scope::Local, l.clone())) as Box<dyn Storage>),
        Box::new(FixedClock(t0())),
    );
    (d, g, l, svc)
}

pub mod cli;
#[cfg(unix)]
pub mod pty;
