//! CLI 계층: clap 정의, 커맨드 핸들러, 출력, 종료 코드.
//! 핸들러는 인자를 `PromptService` 호출로 옮기고 결과를 출력할 뿐이다 (규칙은 core 에 있다).

pub mod args;
pub mod commands;
pub mod context;
pub mod exit;
pub mod output;

pub use args::{Cli, Command};
pub use context::{CliIo, EditorLauncher, SystemEditor};

use crate::core::error::PhError;
use crate::core::service::PromptService;

/// 커맨드를 실행한다. `service` 는 지연 생성한다 (`init` 은 서비스 없이 동작해야 한다).
pub fn run(
    command: Command,
    io: &mut CliIo,
    service: &dyn Fn() -> Result<PromptService, PhError>,
) -> Result<(), PhError> {
    match command {
        Command::Tui => Err(PhError::Usage(
            "TUI 는 이 진입점에서 실행할 수 없습니다. `ph --help` 로 CLI 사용법을 확인하세요"
                .to_string(),
        )),
        Command::Init(a) => commands::init::run(a, io),
        Command::Add(a) => commands::add::run(a, &service()?, io),
        Command::Get(a) => commands::get::run(a, &service()?, io),
        Command::List(a) => commands::list::run(a, &service()?, io),
        Command::Search(a) => commands::search::run(a, &service()?, io),
        Command::Edit(a) => commands::edit::run(a, &service()?, io),
        Command::Rm(a) => commands::rm::run(a, &service()?, io),
        Command::Move(a) => commands::move_::run(a, &service()?, io),
    }
}
