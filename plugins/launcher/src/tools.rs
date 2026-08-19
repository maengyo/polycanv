//! 도구 정의와 설정 파싱.

use std::collections::BTreeMap;

use polycanv_protocol::ToolKind;

/// 실행 가능한 도구 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tool {
    /// 사용자에게 보이는 이름. 설정 키의 `tool_` 뒤부터.
    pub name: String,
    /// 실행 파일.
    pub command: String,
    /// 실행 파일에 넘길 인자.
    pub args: Vec<String>,
    /// 이 도구와 **함께 띄울** 보조 프로세스. 없으면 `None`.
    ///
    /// opencode 처럼 상태를 훅이 아니라 스트림으로 내는 도구는 브리지가 딸려야 한다.
    /// 그걸 런처에 특별 분기로 박지 않고 설정으로 표현한다 — `sidecar_<이름>`.
    /// 새 도구가 브리지를 필요로 해도 **코드를 고칠 필요가 없다.**
    pub sidecar: Option<Sidecar>,
}

/// 도구와 짝지어 뜨는 보조 프로세스.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sidecar {
    pub command: String,
    pub args: Vec<String>,
}

/// 사이드카 인자에서 치환되는 자리표시자 — 방금 뜬 도구 패인의 id.
///
/// 브리지는 "어느 패인의 상태인지" 를 알아야 하는데, 그건 패인이 뜨기 전에는 모른다.
pub const PANE_PLACEHOLDER: &str = "{pane}";

impl Sidecar {
    /// `{pane}` 를 실제 패인 id 로 바꾼 인자 목록.
    pub fn args_for(&self, pane_id: u32) -> Vec<String> {
        let pane = pane_id.to_string();
        self.args
            .iter()
            .map(|a| a.replace(PANE_PLACEHOLDER, &pane))
            .collect()
    }
}

impl Tool {
    /// 이 도구가 어떤 종류인지. 상태 감지가 어떤 전략을 쓸지 고를 때 쓴다.
    ///
    /// **이름이 아니라 실행 파일로 판단한다.** 사용자가 항목 이름을 "내 클로드"로 바꿔도
    /// claude code 라는 사실은 변하지 않는다.
    pub fn kind(&self) -> ToolKind {
        match self.command.rsplit('/').next().unwrap_or(&self.command) {
            "claude" => ToolKind::ClaudeCode,
            "codex" => ToolKind::Codex,
            "opencode" => ToolKind::OpenCode,
            "qwen" => ToolKind::QwenCode,
            sh @ ("bash" | "zsh" | "sh" | "fish" | "pwsh" | "powershell" | "powershell.exe") => {
                ToolKind::Shell(sh.to_string())
            }
            other => ToolKind::Other(other.to_string()),
        }
    }
}

/// 접두사. 이게 붙은 설정 키만 도구로 본다 — 다른 설정과 섞여도 안전하게 골라낸다.
const PREFIX: &str = "tool_";

/// 사이드카 접두사. `sidecar_<도구이름>` 으로 도구와 짝지어진다.
const SIDECAR_PREFIX: &str = "sidecar_";

/// 플러그인 설정에서 도구 목록을 읽는다.
///
/// 값은 `실행파일 인자...` 형태의 한 줄이다. 공백으로 나눈다 —
/// **따옴표로 감싼 인자는 지원하지 않는다.** 필요해지면 그때 넓힌다(YAGNI).
/// 빈 값은 버린다. 순서는 이름 오름차순으로 안정적이다(BTreeMap).
pub fn parse_tools(configuration: &BTreeMap<String, String>) -> Vec<Tool> {
    configuration
        .iter()
        .filter_map(|(key, value)| {
            let name = key.strip_prefix(PREFIX)?;
            let (command, args) = split_command(value)?;
            Some(Tool {
                name: name.to_string(),
                command,
                args,
                sidecar: configuration
                    .get(&format!("{SIDECAR_PREFIX}{name}"))
                    .and_then(|v| split_command(v))
                    .map(|(command, args)| Sidecar { command, args }),
            })
        })
        .collect()
}

/// `실행파일 인자...` 한 줄을 쪼갠다. 빈 값이면 `None`.
fn split_command(value: &str) -> Option<(String, Vec<String>)> {
    let mut parts = value.split_whitespace();
    let command = parts.next()?.to_string();
    Some((command, parts.map(str::to_string).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn 접두사가_붙은_키만_도구로_읽는다() {
        let tools = parse_tools(&cfg(&[
            ("tool_claude", "claude"),
            ("theme", "dark"),
            ("tool_codex", "codex"),
        ]));
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "claude");
        assert_eq!(tools[1].name, "codex");
    }

    #[test]
    fn 인자를_분리한다() {
        let tools = parse_tools(&cfg(&[("tool_pwsh", "pwsh -NoLogo -File x.ps1")]));
        assert_eq!(tools[0].command, "pwsh");
        assert_eq!(tools[0].args, vec!["-NoLogo", "-File", "x.ps1"]);
    }

    #[test]
    fn 빈_값은_버린다() {
        assert!(parse_tools(&cfg(&[("tool_broken", "   ")])).is_empty());
    }

    #[test]
    fn 설정에_없는_도구도_1급_시민이다() {
        // 내장 프리셋이 아니어도 목록에 뜨고 실행돼야 한다.
        let tools = parse_tools(&cfg(&[("tool_내도구", "my-cli --flag")]));
        assert_eq!(tools[0].name, "내도구");
        assert_eq!(tools[0].kind(), ToolKind::Other("my-cli".into()));
    }

    #[test]
    fn 종류는_이름이_아니라_실행파일로_판단한다() {
        let t = Tool {
            name: "내 클로드".into(),
            command: "/opt/bin/claude".into(),
            args: vec![],
            sidecar: None,
        };
        assert_eq!(t.kind(), ToolKind::ClaudeCode);
    }

    #[test]
    fn 사이드카는_도구와_이름으로_짝지어진다() {
        let tools = parse_tools(&cfg(&[
            ("tool_opencode", "opencode --port 47311"),
            ("sidecar_opencode", "bridge.sh --port 47311 --pane {pane}"),
            ("tool_claude", "claude"),
        ]));
        let oc = tools.iter().find(|t| t.name == "opencode").unwrap();
        let sc = oc.sidecar.as_ref().expect("사이드카가 붙어야 한다");
        assert_eq!(sc.command, "bridge.sh");

        // 훅으로 상태를 내는 도구는 사이드카가 필요 없다.
        let cl = tools.iter().find(|t| t.name == "claude").unwrap();
        assert!(cl.sidecar.is_none());
    }

    #[test]
    fn 사이드카는_패인_id_를_치환받는다() {
        // 브리지는 "어느 패인의 상태인지" 를 알아야 하는데 패인이 뜨기 전에는 모른다.
        let sc = Sidecar {
            command: "bridge.sh".into(),
            args: vec![
                "--pane".into(),
                "{pane}".into(),
                "--port".into(),
                "47311".into(),
            ],
        };
        assert_eq!(sc.args_for(7), vec!["--pane", "7", "--port", "47311"]);
    }

    #[test]
    fn 짝_없는_사이드카는_무시된다() {
        // 오타로 sidecar_ 만 남으면 조용히 버린다 — 도구 없이 브리지만 띄우면 의미가 없다.
        let tools = parse_tools(&cfg(&[("sidecar_없는도구", "bridge.sh")]));
        assert!(tools.is_empty());
    }

    #[test]
    fn 셸은_셸로_분류된다() {
        let t = Tool {
            name: "win".into(),
            command: "pwsh".into(),
            args: vec![],
            sidecar: None,
        };
        assert_eq!(t.kind(), ToolKind::Shell("pwsh".into()));
        assert!(
            !t.kind().has_approval_flow(),
            "셸에 🟡 를 켜면 진짜 승인 요청이 묻힌다"
        );
    }
}
