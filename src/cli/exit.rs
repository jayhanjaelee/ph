//! 에러 → 종료 코드, 에러 출력. 종료 코드 매핑은 여기 한 곳에만 둔다.

use std::io::{ErrorKind, Write};
use std::process::ExitCode;

use serde::Serialize;

use super::output::SCHEMA_VERSION;
use crate::core::error::PhError;

/// 종료 코드: 0 성공, 1 일반 오류, 2 사용법 오류, 3 대상 없음.
pub fn exit_code(e: &PhError) -> u8 {
    match e {
        PhError::NotFound { .. } => 3,
        PhError::Usage(_) | PhError::NonInteractive(_) => 2,
        PhError::Io { source, .. } if source.kind() == ErrorKind::BrokenPipe => 0,
        PhError::InvalidId { .. }
        | PhError::InvalidFormat { .. }
        | PhError::AlreadyExists { .. }
        | PhError::LocalNotInitialized
        | PhError::MissingVariable { .. }
        | PhError::Io { .. }
        | PhError::Editor(_)
        | PhError::Clipboard(_) => 1,
    }
}

/// JSON 에러의 `kind`.
pub fn error_kind(e: &PhError) -> &'static str {
    match e {
        PhError::NotFound { .. } => "not_found",
        PhError::InvalidId { .. } => "invalid_id",
        PhError::InvalidFormat { .. } => "invalid_format",
        PhError::AlreadyExists { .. } => "already_exists",
        PhError::LocalNotInitialized => "local_not_initialized",
        PhError::MissingVariable { .. } => "missing_variable",
        PhError::Usage(_) => "usage",
        PhError::NonInteractive(_) => "non_interactive",
        PhError::Io { .. } => "io",
        PhError::Editor(_) => "editor",
        PhError::Clipboard(_) => "clipboard",
    }
}

#[derive(Serialize)]
struct ErrorEnvelope<'a> {
    schema_version: u32,
    error: ErrorBody<'a>,
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    kind: &'static str,
    message: &'a str,
}

/// 에러를 stderr 로 출력하고 종료 코드를 돌려준다. `BrokenPipe` 는 조용히 0 이다.
pub fn report_error(e: &PhError, json: bool, stderr: &mut dyn Write) -> ExitCode {
    let code = exit_code(e);
    if matches!(e, PhError::Io { source, .. } if source.kind() == ErrorKind::BrokenPipe) {
        return ExitCode::from(code);
    }
    let message = e.to_string();
    if json {
        let env = ErrorEnvelope {
            schema_version: SCHEMA_VERSION,
            error: ErrorBody {
                kind: error_kind(e),
                message: &message,
            },
        };
        if let Ok(line) = serde_json::to_string(&env) {
            let _ = writeln!(stderr, "{line}");
            return ExitCode::from(code);
        }
    }
    let _ = writeln!(stderr, "오류: {message}");
    ExitCode::from(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping_table() {
        let io = |k| PhError::io("x", std::io::Error::from(k));
        assert_eq!(exit_code(&PhError::NotFound { id: "a".into() }), 3);
        assert_eq!(exit_code(&PhError::Usage("u".into())), 2);
        assert_eq!(exit_code(&PhError::NonInteractive("u".into())), 2);
        assert_eq!(exit_code(&PhError::LocalNotInitialized), 1);
        assert_eq!(exit_code(&PhError::Editor("e".into())), 1);
        assert_eq!(exit_code(&io(ErrorKind::NotFound)), 1);
        assert_eq!(exit_code(&io(ErrorKind::BrokenPipe)), 0);
    }

    #[test]
    fn json_error_shape() {
        let mut buf = Vec::new();
        report_error(&PhError::LocalNotInitialized, true, &mut buf);
        let v: serde_json::Value = serde_json::from_slice(&buf).unwrap();
        assert_eq!(v["schema_version"], 1);
        assert_eq!(v["error"]["kind"], "local_not_initialized");
    }
}
