//! 플랫폼별 경로. `XDG_*`, `HOME` 은 여기(와 `directories`)에서만 다룬다.

use std::io;
use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs};

/// 해석된 디렉터리들.
#[derive(Debug, Clone)]
pub struct Dirs {
    /// global prompt 디렉터리
    pub global_prompts: PathBuf,
    /// 홈 디렉터리 (local 탐색의 정지 지점). 알 수 없으면 `None`
    pub home: Option<PathBuf>,
}

/// global 경로를 정한다. `ph_home_override`(`PH_HOME`/`--home`)가 있으면 그 아래 `prompts/`,
/// 없으면 OS 데이터 디렉터리 아래 `ph/prompts/` 다.
pub fn resolve_dirs(ph_home_override: Option<&Path>) -> io::Result<Dirs> {
    let home = BaseDirs::new().map(|b| b.home_dir().to_path_buf());
    let global_prompts = match ph_home_override {
        Some(p) => p.join("prompts"),
        None => ProjectDirs::from("", "", "ph")
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "홈 디렉터리를 찾을 수 없습니다. PH_HOME 이나 --home 으로 경로를 지정하세요",
                )
            })?
            .data_dir()
            .join("prompts"),
    };
    Ok(Dirs {
        global_prompts,
        home,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins() {
        let d = resolve_dirs(Some(Path::new("base"))).unwrap();
        assert_eq!(d.global_prompts, Path::new("base").join("prompts"));
    }
}
