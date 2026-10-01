//! `ph search`

use crate::cli::args::SearchArgs;
use crate::cli::context::CliIo;
use crate::cli::output::render_entries;
use crate::core::error::PhError;
use crate::core::service::PromptService;

/// 검색 결과를 출력한다. 결과가 없어도 종료 코드는 0 이다.
pub fn run(args: SearchArgs, svc: &PromptService, io: &mut CliIo) -> Result<(), PhError> {
    let entries = svc.search(&args.query, args.scope.filter(), args.tag.as_deref())?;
    let out = render_entries(&entries, &[], args.json)?;
    io.out(&out)
}
