//! `ph get`

use crate::cli::args::GetArgs;
use crate::cli::context::CliIo;
use crate::cli::output::{ambiguous_warning, to_json_line, GetJson, PromptJson, SCHEMA_VERSION};
use crate::core::error::PhError;
use crate::core::service::PromptService;

/// 본문을 stdout 으로 출력한다 (저장된 그대로, 개행을 덧붙이지 않는다).
pub fn run(args: GetArgs, svc: &PromptService, io: &mut CliIo) -> Result<(), PhError> {
    let r = svc.get(&args.id, args.scope.filter())?;
    if r.ambiguous {
        io.info(&ambiguous_warning(r.prompt.id.as_str()));
    }
    if args.json {
        let line = to_json_line(&GetJson {
            schema_version: SCHEMA_VERSION,
            prompt: PromptJson::new(&r.prompt),
            ambiguous: r.ambiguous,
        })?;
        io.out(&line)
    } else {
        io.out(&r.prompt.body)
    }
}
