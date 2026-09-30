//! 프로세스 환경 조회. 환경 접근은 여기로 모아 다른 계층이 직접 읽지 않게 한다.

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;

/// 현재 디렉터리.
pub fn cwd() -> io::Result<PathBuf> {
    std::env::current_dir()
}

/// 환경변수 조회 (`EDITOR`, `VISUAL` 등을 주입하는 클로저의 실제 구현).
pub fn get(key: &str) -> Option<OsString> {
    std::env::var_os(key)
}
