//! `ph` 진입점: 인자 파싱 → 환경 수집 → 조립 → 실행 → 종료 코드.

use std::io::IsTerminal;
use std::process::ExitCode;

use clap::Parser;

use ph::bootstrap::{build_runtime, BootstrapInput};
use ph::cli::exit::report_error;
use ph::cli::{run, Cli, CliIo, Command, SystemEditor};
use ph::core::error::PhError;

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(c) => c,
        Err(e) => e.exit(),
    };
    let command = cli.command.unwrap_or(Command::Tui);
    let json = command.wants_json();

    let mut stderr = std::io::stderr();
    let cwd = match ph::platform::env::cwd() {
        Ok(p) => p,
        Err(e) => {
            return report_error(&PhError::io("현재 디렉터리 확인", e), json, &mut stderr);
        }
    };
    if matches!(command, Command::Tui) {
        eprintln!("TUI 는 아직 구현되지 않았습니다. `ph --help` 로 CLI 사용법을 확인하세요");
        return ExitCode::from(1);
    }

    let interactive = std::io::stdin().is_terminal();
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    let mut err_out = std::io::stderr().lock();
    let env = |k: &str| ph::platform::env::get(k);
    let home = cli.home.as_deref();
    let mut io = CliIo {
        stdin: &mut stdin,
        stdout: &mut stdout,
        stderr: &mut err_out,
        interactive,
        cwd: &cwd,
        home_override: home,
        env: &env,
        editor: &SystemEditor,
    };
    let input = BootstrapInput {
        home_override: home,
        cwd: &cwd,
    };
    let result = run(command, &mut io, &|| {
        build_runtime(&input).map(|r| r.service)
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => report_error(&e, json, &mut stderr),
    }
}
