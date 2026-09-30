//! `ph move`

use crate::cli::args::MoveArgs;
use crate::cli::context::CliIo;
use crate::cli::output::{badge, to_json_line, MoveJson, PromptJson, SCHEMA_VERSION};
use crate::core::error::PhError;
use crate::core::service::PromptService;

/// scope 를 옮긴다.
pub fn run(args: MoveArgs, svc: &PromptService, io: &mut CliIo) -> Result<(), PhError> {
    let to = args.to.into();
    let w = svc.move_to(&args.id, to)?;
    let from = w.scope.other();
    io.info(&format!(
        "이동됨: {} {} → {}",
        badge(from),
        w.prompt.id,
        badge(w.scope)
    ));
    if args.json {
        let line = to_json_line(&MoveJson {
            schema_version: SCHEMA_VERSION,
            prompt: PromptJson::new(&w.prompt),
            from: from.as_str(),
            to: w.scope.as_str(),
        })?;
        io.out(&line)
    } else {
        io.out(&format!("{}\n", w.prompt.id))
    }
}
