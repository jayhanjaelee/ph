//! 파일시스템 저장소. 디렉터리 하나가 scope 하나다.

use std::io;
use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

use crate::core::error::PhError;
use crate::core::format;
use crate::core::model::{Prompt, PromptId, Scope};
use crate::core::storage::{Listing, SkippedEntry, Storage};
use crate::platform;

const EXT: &str = "md";
/// local 저장소 디렉터리 이름.
pub const LOCAL_DIR: &str = ".ph";
/// `.ph/` 아래 prompt 디렉터리 이름.
pub const PROMPTS_DIR: &str = "prompts";

/// `<id>.md` 파일들을 담은 디렉터리 하나를 다루는 저장소.
#[derive(Debug)]
pub struct FsStorage {
    scope: Scope,
    dir: PathBuf,
}

impl FsStorage {
    /// `dir` 를 prompt 디렉터리로 쓰는 저장소. 디렉터리는 첫 `put` 때 만들어진다.
    pub fn new(scope: Scope, dir: PathBuf) -> Self {
        Self { scope, dir }
    }

    fn path_of(&self, id: &PromptId) -> PathBuf {
        self.dir.join(format!("{}.{EXT}", id.as_str()))
    }

    /// id 에 해당하는 실제 파일 경로. 정규화 형태만 다른 파일명(예: macOS 에서 만든 NFD)이
    /// 있으면 그 경로를 돌려주고, 없으면 NFC 기준 경로를 돌려준다.
    fn resolve_path(&self, id: &PromptId) -> PathBuf {
        let exact = self.path_of(id);
        if exact.is_file() {
            return exact;
        }
        if let Ok(rd) = std::fs::read_dir(&self.dir) {
            for entry in rd.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some(EXT) {
                    continue;
                }
                let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let nfc: String = stem.nfc().collect();
                if nfc == id.as_str() && path.is_file() {
                    return path;
                }
            }
        }
        exact
    }

    fn read_file(&self, path: &Path, id: &PromptId) -> Result<Prompt, PhError> {
        let bytes =
            std::fs::read(path).map_err(|e| PhError::io(format!("{} 읽기", path.display()), e))?;
        let raw = String::from_utf8(bytes).map_err(|_| PhError::InvalidFormat {
            id: Some(id.as_str().to_string()),
            line: None,
            reason: "UTF-8 이 아닙니다. 파일을 UTF-8 로 저장하세요".to_string(),
        })?;
        format::parse(&raw, id, self.scope)
    }
}

impl Storage for FsStorage {
    fn scope(&self) -> Scope {
        self.scope
    }

    fn location(&self) -> String {
        self.dir.display().to_string()
    }

    fn list(&self) -> Result<Listing, PhError> {
        let mut out = Listing::default();
        let rd = match std::fs::read_dir(&self.dir) {
            Ok(rd) => rd,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(PhError::io(format!("{} 읽기", self.dir.display()), e)),
        };
        for entry in rd {
            let entry =
                entry.map_err(|e| PhError::io(format!("{} 읽기", self.dir.display()), e))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some(EXT) || !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let id = match PromptId::parse(stem) {
                Ok(id) => id,
                Err(e) => {
                    out.skipped.push(SkippedEntry {
                        name,
                        reason: e.to_string(),
                    });
                    continue;
                }
            };
            match self.read_file(&path, &id) {
                Ok(p) => out.prompts.push(p),
                Err(e) => out.skipped.push(SkippedEntry {
                    name,
                    reason: e.to_string(),
                }),
            }
        }
        out.prompts.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        Ok(out)
    }

    fn get(&self, id: &PromptId) -> Result<Option<Prompt>, PhError> {
        let path = self.resolve_path(id);
        if !path.is_file() {
            return Ok(None);
        }
        self.read_file(&path, id).map(Some)
    }

    fn put(&self, prompt: &Prompt) -> Result<(), PhError> {
        let path = self.resolve_path(&prompt.id);
        let text = format::serialize(prompt)?;
        platform::fs::atomic_write(&path, text.as_bytes())
            .map_err(|e| PhError::io(format!("{} 쓰기", path.display()), e))
    }

    fn delete(&self, id: &PromptId) -> Result<(), PhError> {
        let path = self.resolve_path(id);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(PhError::NotFound {
                id: id.as_str().to_string(),
            }),
            Err(e) => Err(PhError::io(format!("{} 삭제", path.display()), e)),
        }
    }
}

/// `cwd` 에서 위로 올라가며 `.ph/` 디렉터리를 찾아 그 경로를 돌려준다 (prompt 디렉터리는
/// `.join("prompts")`). `home` 에 닿으면 멈추며 홈의 `.ph/` 는 인정하지 않는다.
/// 파일시스템 루트에도 닿으면 멈추고 루트의 `.ph/` 는 검사하지 않는다.
pub fn find_local(cwd: &Path, home: Option<&Path>) -> Option<PathBuf> {
    let mut dir = cwd;
    loop {
        if home == Some(dir) {
            return None;
        }
        // 루트(부모 없음)에서는 검사하지 않고 멈춘다.
        dir.parent()?;
        let candidate = dir.join(LOCAL_DIR);
        if candidate.is_dir() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}

/// `init_local` 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOutcome {
    /// `.ph` 디렉터리 경로
    pub ph_dir: PathBuf,
    /// 새로 만들었으면 `true`, 이미 있었으면 `false`
    pub created: bool,
    /// 상위 디렉터리에서 발견한 다른 local 저장소(`.ph`)
    pub parent_local: Option<PathBuf>,
}

/// `cwd` 에 `.ph/prompts/` (와 빈 `.gitkeep`)를 만든다. `cwd` 가 홈이면 `Usage` 에러다.
/// 이미 있으면 그대로 두고 `created = false` 를 돌려준다.
pub fn init_local(cwd: &Path, home: Option<&Path>) -> Result<InitOutcome, PhError> {
    if home == Some(cwd) {
        return Err(PhError::Usage(
            "홈 디렉터리에는 local 저장소를 만들 수 없습니다. 프로젝트 디렉터리로 이동한 뒤 `ph init` 을 실행하세요"
                .to_string(),
        ));
    }
    let ph_dir = cwd.join(LOCAL_DIR);
    let prompts = ph_dir.join(PROMPTS_DIR);
    let created = !ph_dir.is_dir();
    let already_ready = prompts.is_dir();
    std::fs::create_dir_all(&prompts)
        .map_err(|e| PhError::io(format!("{} 만들기", prompts.display()), e))?;
    if !already_ready {
        let keep = prompts.join(".gitkeep");
        if !keep.exists() {
            std::fs::write(&keep, b"")
                .map_err(|e| PhError::io(format!("{} 쓰기", keep.display()), e))?;
        }
    }
    let parent_local = cwd.parent().and_then(|p| find_local(p, home));
    Ok(InitOutcome {
        ph_dir,
        created,
        parent_local,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;

    fn prompt(id: &str) -> Prompt {
        let t = DateTime::parse_from_rfc3339("2026-09-30T12:00:00+09:00").unwrap();
        Prompt {
            id: PromptId::parse(id).unwrap(),
            scope: Scope::Global,
            title: id.into(),
            body: "본문\n".into(),
            tags: vec![],
            description: None,
            created_at: t,
            updated_at: t,
        }
    }

    #[test]
    fn put_get_list_delete() {
        let d = tempfile::tempdir().unwrap();
        let s = FsStorage::new(Scope::Global, d.path().join("prompts"));
        assert!(s.list().unwrap().prompts.is_empty());
        s.put(&prompt("코드-리뷰")).unwrap();
        let got = s
            .get(&PromptId::parse("코드-리뷰").unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(got.body, "본문\n");
        assert_eq!(s.list().unwrap().prompts.len(), 1);
        s.delete(&got.id).unwrap();
        assert!(matches!(s.delete(&got.id), Err(PhError::NotFound { .. })));
    }

    #[test]
    fn broken_file_is_skipped() {
        let d = tempfile::tempdir().unwrap();
        let s = FsStorage::new(Scope::Local, d.path().to_path_buf());
        s.put(&prompt("ok")).unwrap();
        std::fs::write(d.path().join("bad.md"), "no frontmatter").unwrap();
        std::fs::write(d.path().join("notes.txt"), "x").unwrap();
        let l = s.list().unwrap();
        assert_eq!(l.prompts.len(), 1);
        assert_eq!(l.skipped.len(), 1);
        assert_eq!(l.prompts[0].scope, Scope::Local);
    }

    #[test]
    fn find_local_walks_up_and_stops_at_home() {
        let d = tempfile::tempdir().unwrap();
        let home = d.path().join("home");
        let proj = home.join("proj");
        let deep = proj.join("a").join("b");
        std::fs::create_dir_all(&deep).unwrap();
        // 프로젝트에 .ph 가 없고 홈에만 있으면 인정하지 않는다.
        std::fs::create_dir_all(home.join(LOCAL_DIR)).unwrap();
        assert_eq!(find_local(&deep, Some(&home)), None);
        std::fs::create_dir_all(proj.join(LOCAL_DIR)).unwrap();
        assert_eq!(find_local(&deep, Some(&home)), Some(proj.join(LOCAL_DIR)));
        // 홈 밖에서는 루트까지 올라간다.
        assert_eq!(
            find_local(&deep, Some(&d.path().join("other"))),
            Some(proj.join(LOCAL_DIR))
        );
    }
}
