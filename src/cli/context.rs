//! 핸들러가 쓰는 입출력 컨텍스트. 테스트에서 가짜로 바꿀 수 있다.

use std::ffi::OsString;
use std::io::{self, Read, Write};
use std::path::Path;

use crate::core::error::PhError;
use crate::platform::editor::{run_editor, EditorCommand, EditorExit};

/// 에디터 실행기. 테스트에서 가짜로 대체한다.
pub trait EditorLauncher {
    /// `cmd` 로 `file` 을 열고 종료를 기다린다.
    fn run(&self, cmd: &EditorCommand, file: &Path) -> io::Result<EditorExit>;
}

/// `platform::editor::run_editor` 에 위임하는 실제 실행기.
pub struct SystemEditor;

impl EditorLauncher for SystemEditor {
    fn run(&self, cmd: &EditorCommand, file: &Path) -> io::Result<EditorExit> {
        run_editor(cmd, file)
    }
}

/// CLI 입출력과 환경.
pub struct CliIo<'a> {
    /// 표준 입력
    pub stdin: &'a mut dyn Read,
    /// 표준 출력 (데이터만)
    pub stdout: &'a mut dyn Write,
    /// 표준 에러 (안내, 경고, 에러)
    pub stderr: &'a mut dyn Write,
    /// stdin 이 터미널인가. false 면 확인 프롬프트를 띄우지 않는다
    pub interactive: bool,
    /// 현재 디렉터리
    pub cwd: &'a Path,
    /// `--home` / `PH_HOME` (init 이 홈 디렉터리를 판별할 때 쓴다)
    pub home_override: Option<&'a Path>,
    /// 환경변수 조회 (에디터 결정용)
    pub env: &'a dyn Fn(&str) -> Option<OsString>,
    /// 에디터 실행기
    pub editor: &'a dyn EditorLauncher,
}

impl CliIo<'_> {
    /// stdout 에 그대로 쓴다.
    pub fn out(&mut self, s: &str) -> Result<(), PhError> {
        self.stdout
            .write_all(s.as_bytes())
            .and_then(|_| self.stdout.flush())
            .map_err(|e| PhError::io("표준 출력 쓰기", e))
    }

    /// stderr 에 한 줄 쓴다. stderr 쓰기 실패는 무시한다.
    pub fn info(&mut self, line: &str) {
        let _ = writeln!(self.stderr, "{line}");
    }

    /// stderr 에 프롬프트를 (개행 없이) 쓴다.
    pub fn prompt(&mut self, text: &str) {
        let _ = write!(self.stderr, "{text}");
        let _ = self.stderr.flush();
    }

    /// stdin 에서 한 줄을 읽는다 (개행 제외). EOF 면 빈 문자열.
    pub fn read_line(&mut self) -> Result<String, PhError> {
        let mut buf = Vec::new();
        let mut byte = [0u8; 1];
        loop {
            match self.stdin.read(&mut byte) {
                Ok(0) => break,
                Ok(_) if byte[0] == b'\n' => break,
                Ok(_) => buf.push(byte[0]),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(PhError::io("표준 입력 읽기", e)),
            }
        }
        Ok(String::from_utf8_lossy(&buf).trim().to_string())
    }
}
