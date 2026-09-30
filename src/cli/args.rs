//! clap 정의. 로직은 없다.

use std::path::PathBuf;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

use crate::core::model::Scope;
use crate::core::service::{ScopeFilter, WriteTarget};

/// 최상위 인자.
#[derive(Debug, Parser)]
#[command(name = "ph", version, about = "Prompt Hub: AI Agent 용 prompt 저장소")]
pub struct Cli {
    /// global 저장소의 기준 디렉터리 (그 아래 prompts/). local 경로에는 영향이 없다
    #[arg(long, global = true, env = "PH_HOME", value_name = "PATH")]
    pub home: Option<PathBuf>,
    /// 없으면 TUI 를 실행한다
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// 서브커맨드.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// TUI 실행 (인자 없이 `ph` 와 같다)
    Tui,
    /// 현재 디렉터리에 local 저장소(.ph/)를 만든다
    Init(InitArgs),
    /// prompt 를 추가한다
    Add(AddArgs),
    /// prompt 본문을 stdout 으로 출력한다
    Get(GetArgs),
    /// prompt 목록
    List(ListArgs),
    /// prompt 검색
    Search(SearchArgs),
    /// $VISUAL/$EDITOR 로 편집한다
    Edit(EditArgs),
    /// prompt 를 삭제한다
    Rm(RmArgs),
    /// prompt 의 scope 를 옮긴다
    Move(MoveArgs),
}

impl Command {
    /// `--json` 이 요청되었는지 (에러 출력 형식 결정용).
    pub fn wants_json(&self) -> bool {
        match self {
            Command::Tui | Command::Edit(_) => false,
            Command::Init(a) => a.json,
            Command::Add(a) => a.json,
            Command::Get(a) => a.json,
            Command::List(a) => a.json,
            Command::Search(a) => a.json,
            Command::Rm(a) => a.json,
            Command::Move(a) => a.json,
        }
    }
}

/// `--local` / `--global`.
#[derive(Debug, Args, Clone, Copy, Default)]
pub struct ScopeArgs {
    /// local 저장소만 대상으로 한다
    #[arg(long, conflicts_with = "global")]
    pub local: bool,
    /// global 저장소만 대상으로 한다
    #[arg(long)]
    pub global: bool,
}

impl ScopeArgs {
    /// 읽기와 읽기 규칙으로 대상을 찾는 쓰기: 플래그가 없으면 `All`.
    pub fn filter(&self) -> ScopeFilter {
        if self.local {
            ScopeFilter::Only(Scope::Local)
        } else if self.global {
            ScopeFilter::Only(Scope::Global)
        } else {
            ScopeFilter::All
        }
    }

    /// `add`: 플래그가 없으면 `Auto`.
    pub fn target(&self) -> WriteTarget {
        match self.filter() {
            ScopeFilter::All => WriteTarget::Auto,
            ScopeFilter::Only(s) => WriteTarget::Explicit(s),
        }
    }
}

/// `ph init`
#[derive(Debug, Args)]
pub struct InitArgs {
    /// JSON 으로 출력
    #[arg(long)]
    pub json: bool,
}

/// `ph add`
#[derive(Debug, Args)]
#[command(group(ArgGroup::new("source").args(["body", "file", "stdin"]).required(true).multiple(false)))]
pub struct AddArgs {
    /// 제목 (id 의 원본)
    pub title: String,
    /// 본문 텍스트
    #[arg(long)]
    pub body: Option<String>,
    /// 본문을 읽을 파일
    #[arg(long, value_name = "PATH")]
    pub file: Option<PathBuf>,
    /// 본문을 stdin 에서 읽는다
    #[arg(long)]
    pub stdin: bool,
    /// 태그 (여러 번 지정 가능)
    #[arg(long = "tag", value_name = "TAG")]
    pub tags: Vec<String>,
    /// 짧은 설명
    #[arg(long = "desc", value_name = "TEXT")]
    pub description: Option<String>,
    #[command(flatten)]
    pub scope: ScopeArgs,
    /// JSON 으로 출력
    #[arg(long)]
    pub json: bool,
}

/// `ph get`
#[derive(Debug, Args)]
pub struct GetArgs {
    /// prompt id
    pub id: String,
    #[command(flatten)]
    pub scope: ScopeArgs,
    /// JSON 으로 출력
    #[arg(long)]
    pub json: bool,
}

/// `ph list`
#[derive(Debug, Args)]
pub struct ListArgs {
    /// 태그로 거른다
    #[arg(long)]
    pub tag: Option<String>,
    #[command(flatten)]
    pub scope: ScopeArgs,
    /// JSON 으로 출력
    #[arg(long)]
    pub json: bool,
}

/// `ph search`
#[derive(Debug, Args)]
pub struct SearchArgs {
    /// 검색어
    pub query: String,
    /// 태그로 거른다
    #[arg(long)]
    pub tag: Option<String>,
    #[command(flatten)]
    pub scope: ScopeArgs,
    /// JSON 으로 출력
    #[arg(long)]
    pub json: bool,
}

/// `ph edit`
#[derive(Debug, Args)]
pub struct EditArgs {
    /// prompt id
    pub id: String,
    #[command(flatten)]
    pub scope: ScopeArgs,
}

/// `ph rm`
#[derive(Debug, Args)]
pub struct RmArgs {
    /// prompt id
    pub id: String,
    /// 확인 없이 삭제
    #[arg(long, short = 'y')]
    pub yes: bool,
    #[command(flatten)]
    pub scope: ScopeArgs,
    /// JSON 으로 출력
    #[arg(long)]
    pub json: bool,
}

/// `ph move`
#[derive(Debug, Args)]
pub struct MoveArgs {
    /// prompt id
    pub id: String,
    /// 옮길 scope
    #[arg(long, value_enum)]
    pub to: ScopeArg,
    /// JSON 으로 출력
    #[arg(long)]
    pub json: bool,
}

/// `--to` 값.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ScopeArg {
    /// local
    Local,
    /// global
    Global,
}

impl From<ScopeArg> for Scope {
    fn from(a: ScopeArg) -> Scope {
        match a {
            ScopeArg::Local => Scope::Local,
            ScopeArg::Global => Scope::Global,
        }
    }
}
