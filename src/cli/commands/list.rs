//! `ph list`

use crate::cli::args::ListArgs;
use crate::cli::context::CliIo;
use crate::cli::output::{render_entries, skip_warning};
use crate::core::error::PhError;
use crate::core::service::PromptService;

/// 병합 목록을 출력한다.
pub fn run(args: ListArgs, svc: &PromptService, io: &mut CliIo) -> Result<(), PhError> {
    let r = svc.list(args.scope.filter(), args.tag.as_deref())?;
    for s in &r.skipped {
        io.info(&skip_warning(s));
    }
    let out = render_entries(&r.entries, &r.skipped, args.json)?;
    io.out(&out)
}
