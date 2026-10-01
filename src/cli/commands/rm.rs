//! `ph rm`

use crate::cli::args::RmArgs;
use crate::cli::context::CliIo;
use crate::cli::output::{
    ambiguous_warning, badge, to_json_line, RemovedJson, RmJson, SCHEMA_VERSION,
};
use crate::core::error::PhError;
use crate::core::service::{PromptService, ScopeFilter};

/// prompt 를 삭제한다. 확인한 항목과 실제로 지우는 항목은 같다 (scope 고정).
pub fn run(args: RmArgs, svc: &PromptService, io: &mut CliIo) -> Result<(), PhError> {
    let r = svc.target(&args.id, args.scope.filter())?;
    let prompt = r.prompt;
    if r.ambiguous {
        io.info(&ambiguous_warning(prompt.id.as_str()));
    }
    if !args.yes {
        if !io.interactive {
            return Err(PhError::NonInteractive(
                "삭제하려면 --yes 를 지정하세요".to_string(),
            ));
        }
        io.prompt(&format!(
            "{} {} ({}) 를 삭제할까요? [y/N] ",
            badge(prompt.scope),
            prompt.title,
            prompt.id
        ));
        let answer = io.read_line()?.to_lowercase();
        if answer != "y" && answer != "yes" {
            io.info("취소했습니다");
            return Ok(());
        }
    }
    let scope = svc.remove(prompt.id.as_str(), ScopeFilter::Only(prompt.scope))?;
    io.info(&format!("삭제됨: {} {}", badge(scope), prompt.id));
    if r.ambiguous {
        io.info(&format!(
            "참고: {} 에 같은 id 가 남아 있습니다",
            scope.other().as_str()
        ));
    }
    if args.json {
        let line = to_json_line(&RmJson {
            schema_version: SCHEMA_VERSION,
            removed: RemovedJson {
                id: prompt.id.as_str().to_string(),
                scope: scope.as_str(),
            },
        })?;
        io.out(&line)?;
    }
    Ok(())
}
