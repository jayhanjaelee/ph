//! 프로젝트 공통 에러 타입.

use thiserror::Error;

/// `ph` 전체에서 쓰는 에러. 종료 코드 매핑은 `cli` 가 한다.
#[derive(Debug, Error)]
pub enum PhError {
    /// 대상 prompt 가 없다.
    #[error("id '{id}' 에 해당하는 prompt 가 없습니다. `ph list` 로 id 를 확인하세요")]
    NotFound {
        /// 찾으려던 id
        id: String,
    },
    /// id 규칙 위반.
    #[error("유효하지 않은 id '{input}': {reason}")]
    InvalidId {
        /// 입력값
        input: String,
        /// 위반 사유와 해결 방법
        reason: String,
    },
    /// frontmatter 등 파일 형식 오류.
    #[error("{}", format_invalid(.id, .line, .reason))]
    InvalidFormat {
        /// 관련 id (알 수 있을 때)
        id: Option<String>,
        /// 오류 줄 번호 (1부터, 알 수 있을 때)
        line: Option<usize>,
        /// 사유
        reason: String,
    },
    /// 같은 id 가 이미 있다.
    #[error("id '{id}' 는 이미 존재합니다. 다른 제목을 쓰거나 기존 prompt 를 수정하세요")]
    AlreadyExists {
        /// 충돌한 id
        id: String,
    },
    /// local 저장소가 없다.
    #[error("local 저장소가 없습니다. `ph init` 으로 만든 뒤 다시 시도하세요")]
    LocalNotInitialized,
    /// 값이 주어지지 않은 템플릿 변수.
    #[error("변수 '{name}' 의 값이 없습니다. --var {name}=<값> 으로 지정하거나 --allow-missing 을 쓰세요")]
    MissingVariable {
        /// 변수 이름
        name: String,
    },
    /// 사용법 오류.
    #[error("{0}")]
    Usage(String),
    /// 비대화형 환경에서 확인이 필요한 동작.
    #[error("{0}")]
    NonInteractive(String),
    /// IO 실패. 항상 어떤 동작 또는 경로였는지 `context` 를 붙인다.
    #[error("{context}: {source}")]
    Io {
        /// 동작 또는 경로 설명
        context: String,
        /// 원인
        #[source]
        source: std::io::Error,
    },
    /// 외부 에디터 실행 실패.
    #[error("{0}")]
    Editor(String),
    /// 클립보드 실패.
    #[error("{0}")]
    Clipboard(String),
}

impl PhError {
    /// `io::Error` 에 context 를 붙여 `PhError::Io` 로 만든다.
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        PhError::Io {
            context: context.into(),
            source,
        }
    }
}

fn format_invalid(id: &Option<String>, line: &Option<usize>, reason: &str) -> String {
    let mut s = String::from("파일 형식 오류");
    if let Some(id) = id {
        s.push_str(&format!(" ({id})"));
    }
    if let Some(line) = line {
        s.push_str(&format!(" {line}번째 줄"));
    }
    s.push_str(&format!(": {reason}"));
    s
}
