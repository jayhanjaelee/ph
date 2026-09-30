//! 조립 지점: 경로, 저장소, 시계를 묶어 `PromptService` 를 만든다. `main` 만 호출한다.

use std::path::{Path, PathBuf};

use crate::core::clock::Clock;
use crate::core::error::PhError;
use crate::core::model::Scope;
use crate::core::service::PromptService;
use crate::core::storage::Storage;
use crate::platform;
use crate::storage::fs::{find_local, FsStorage, PROMPTS_DIR};

/// `build_runtime` 입력.
#[derive(Debug, Clone, Copy)]
pub struct BootstrapInput<'a> {
    /// `--home` / `PH_HOME` (global 경로만 바꾼다)
    pub home_override: Option<&'a Path>,
    /// 현재 디렉터리 (local 탐색 시작점)
    pub cwd: &'a Path,
}

/// 조립 결과.
pub struct Runtime {
    /// 완성된 서비스
    pub service: PromptService,
    /// 해석된 디렉터리
    pub dirs: platform::paths::Dirs,
    /// 찾은 `.ph` 디렉터리 (없으면 `None`)
    pub local_root: Option<PathBuf>,
}

/// 시스템 시계 어댑터.
struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> chrono::DateTime<chrono::FixedOffset> {
        platform::time::now_local()
    }
}

/// 디렉터리를 만들지 않고 서비스를 조립한다.
pub fn build_runtime(input: &BootstrapInput) -> Result<Runtime, PhError> {
    let dirs = platform::paths::resolve_dirs(input.home_override)
        .map_err(|e| PhError::io("global 저장소 경로 결정", e))?;
    let global: Box<dyn Storage> =
        Box::new(FsStorage::new(Scope::Global, dirs.global_prompts.clone()));
    let local_root = find_local(input.cwd, dirs.home.as_deref());
    let local: Option<Box<dyn Storage>> = local_root
        .as_ref()
        .map(|ph| Box::new(FsStorage::new(Scope::Local, ph.join(PROMPTS_DIR))) as Box<dyn Storage>);
    let service = PromptService::new(global, local, Box::new(SystemClock));
    Ok(Runtime {
        service,
        dirs,
        local_root,
    })
}
