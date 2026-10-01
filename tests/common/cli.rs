//! CLI 바이너리 통합 테스트용 격리 환경. 실제 홈/데이터 디렉터리와 사용자 환경변수를 쓰지 않는다.
#![allow(dead_code)]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

/// 실행 결과.
#[derive(Debug)]
pub struct Out {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl Out {
    /// stdout 전체를 JSON 하나로 파싱한다.
    pub fn json(&self) -> Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout 이 JSON 이 아니다: {e}\n{:?}", self.stdout))
    }
    /// stderr 전체를 JSON 하나로 파싱한다 (에러 JSON).
    pub fn err_json(&self) -> Value {
        serde_json::from_str(&self.stderr)
            .unwrap_or_else(|e| panic!("stderr 가 JSON 이 아니다: {e}\n{:?}", self.stderr))
    }
    pub fn stdout_lines(&self) -> Vec<&str> {
        self.stdout.lines().collect()
    }
    pub fn stderr_lines(&self) -> Vec<&str> {
        self.stderr.lines().collect()
    }
    /// 성공(0) 을 assert 하고 자신을 돌려준다.
    pub fn ok(self) -> Self {
        assert_eq!(self.code, 0, "{self:#?}");
        self
    }
    pub fn code(self, want: i32) -> Self {
        assert_eq!(self.code, want, "{self:#?}");
        self
    }
}

/// `root/home/proj` 를 cwd, `root/ph_home` 을 `PH_HOME` 으로 쓰는 샌드박스.
pub struct Sandbox {
    pub root: tempfile::TempDir,
    pub home: PathBuf,
    pub ph_home: PathBuf,
    pub proj: PathBuf,
}

impl Sandbox {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let proj = home.join("proj");
        let ph_home = root.path().join("ph_home");
        std::fs::create_dir_all(&proj).unwrap();
        Sandbox {
            root,
            home,
            ph_home,
            proj,
        }
    }

    /// `ph init` 까지 끝낸 샌드박스.
    pub fn with_local() -> Self {
        let s = Self::new();
        s.run(&["init"]).ok();
        s
    }

    pub fn global_dir(&self) -> PathBuf {
        self.ph_home.join("prompts")
    }
    pub fn local_dir(&self) -> PathBuf {
        self.proj.join(".ph").join("prompts")
    }

    /// 환경을 비운 명령을 만든다.
    pub fn command(&self, cwd: &Path) -> Command {
        let mut c = Command::new(env!("CARGO_BIN_EXE_ph"));
        c.env_clear()
            .env("PH_HOME", &self.ph_home)
            .env("HOME", &self.home)
            .current_dir(cwd);
        c
    }

    pub fn run(&self, args: &[&str]) -> Out {
        self.exec(&self.proj, args, None, &[])
    }

    pub fn run_in(&self, cwd: &Path, args: &[&str]) -> Out {
        self.exec(cwd, args, None, &[])
    }

    pub fn run_stdin(&self, args: &[&str], stdin: &[u8]) -> Out {
        self.exec(&self.proj, args, Some(stdin), &[])
    }

    pub fn run_env(&self, args: &[&str], env: &[(&str, &str)]) -> Out {
        self.exec(&self.proj, args, None, env)
    }

    /// stdin 은 항상 파이프(또는 null)라서 터미널이 아니다.
    pub fn exec(
        &self,
        cwd: &Path,
        args: &[&str],
        stdin: Option<&[u8]>,
        env: &[(&str, &str)],
    ) -> Out {
        let mut c = self.command(cwd);
        c.args(args).envs(env.iter().copied());
        c.stdout(Stdio::piped()).stderr(Stdio::piped());
        c.stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
        let mut child = c.spawn().unwrap();
        if let Some(data) = stdin {
            let mut si = child.stdin.take().unwrap();
            si.write_all(data).unwrap();
        }
        let o = child.wait_with_output().unwrap();
        Out {
            code: o.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&o.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&o.stderr).into_owned(),
        }
    }

    /// 간단한 add 헬퍼 (본문 `<title> 본문`). 저장된 id 를 돌려준다.
    pub fn add(&self, title: &str, scope_flag: Option<&str>) -> String {
        let body = format!("{title} 본문");
        let mut args = vec!["add", title, "--body", &body];
        if let Some(f) = scope_flag {
            args.push(f);
        }
        self.run(&args).ok().stdout.trim_end().to_string()
    }

    /// 파일에 직접 쓴다 (깨진 파일 등).
    pub fn write_file(&self, dir: &Path, name: &str, content: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), content).unwrap();
    }
}

/// 두 경로를 심볼릭 링크 해석 후 비교한다.
pub fn same_path(a: &str, b: &Path) -> bool {
    match (Path::new(a.trim()).canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}
