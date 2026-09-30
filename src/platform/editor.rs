//! 외부 에디터 실행. 셸을 거치지 않고 program/args 를 분리해 실행한다.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

/// 실행할 에디터 명령.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorCommand {
    /// 실행 파일
    pub program: OsString,
    /// 파일 경로 앞에 붙는 인자
    pub args: Vec<OsString>,
}

/// 에디터 종료 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorExit {
    /// 정상 종료 (exit 0)
    Success,
    /// 비정상 종료. 시그널 종료면 `None`
    Failed(Option<i32>),
}

/// `$VISUAL` → `$EDITOR` → `vi` 순으로 에디터를 정한다. 값은 공백으로만 분리한다
/// (따옴표는 지원하지 않는다). 환경변수는 `env` 로 주입받는다.
pub fn resolve_editor(env: &dyn Fn(&str) -> Option<OsString>) -> EditorCommand {
    for key in ["VISUAL", "EDITOR"] {
        let Some(value) = env(key) else { continue };
        let parts: Vec<OsString> = match value.to_str() {
            Some(s) => s.split_whitespace().map(OsString::from).collect(),
            None => vec![value],
        };
        let mut it = parts.into_iter();
        if let Some(program) = it.next() {
            return EditorCommand {
                program,
                args: it.collect(),
            };
        }
    }
    EditorCommand {
        program: OsString::from("vi"),
        args: Vec::new(),
    }
}

/// 에디터로 `file` 을 열고 종료를 기다린다. 실행 자체가 실패하면 `io::Error` 다.
pub fn run_editor(cmd: &EditorCommand, file: &Path) -> io::Result<EditorExit> {
    let status = Command::new(&cmd.program)
        .args(&cmd.args)
        .arg(file)
        .status()?;
    Ok(if status.success() {
        EditorExit::Success
    } else {
        EditorExit::Failed(status.code())
    })
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 에디터에 넘기는 임시 파일. 전용 임시 디렉터리에 만들고 Drop 시 디렉터리째 지운다.
#[derive(Debug)]
pub struct TempEditFile {
    dir: PathBuf,
    path: PathBuf,
}

impl TempEditFile {
    /// `<임시 디렉터리>/ph-<pid>-<n>/ph-<id>.md` 에 `contents` 를 쓴다.
    /// 파일 이름에 id 를 넣는 것은 에디터 제목줄에 보이게 하기 위해서다.
    pub fn create(id: &str, contents: &str) -> io::Result<Self> {
        let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("ph-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(format!("ph-{id}.md"));
        if let Err(e) = std::fs::write(&path, contents) {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(e);
        }
        Ok(Self { dir, path })
    }

    /// 임시 파일 경로.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// 현재 내용을 읽는다. UTF-8 이 아니면 `InvalidData`.
    pub fn read(&self) -> io::Result<String> {
        let bytes = std::fs::read(&self.path)?;
        String::from_utf8(bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "UTF-8 이 아닌 내용"))
    }
}

impl Drop for TempEditFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_of(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<OsString> {
        move |k| {
            pairs
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| OsString::from(*v))
        }
    }

    #[test]
    fn temp_file_roundtrip_and_cleanup() {
        let f = TempEditFile::create("a", "안녕").unwrap();
        let dir = f.path().parent().unwrap().to_path_buf();
        assert_eq!(f.read().unwrap(), "안녕");
        drop(f);
        assert!(!dir.exists());
    }

    #[test]
    fn precedence_and_split() {
        let c = resolve_editor(&env_of(&[("VISUAL", "code --wait"), ("EDITOR", "nano")]));
        assert_eq!(c.program, "code");
        assert_eq!(c.args, vec![OsString::from("--wait")]);
        let c = resolve_editor(&env_of(&[("EDITOR", "nano")]));
        assert_eq!(c.program, "nano");
        let c = resolve_editor(&env_of(&[("VISUAL", "  "), ("EDITOR", "")]));
        assert_eq!(c.program, "vi");
    }
}
