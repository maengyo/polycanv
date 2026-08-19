//! polycanv 런처.
//!
//! 하는 일은 하나다: **도구를 골라 새 패인에서 실행한다.**
//!
//! 도구 목록은 코드에 박지 않는다. 레이아웃의 플러그인 설정 블록에서 읽으므로
//! 사용자가 항목을 추가·수정할 수 있다. 아래 기본 6종은 **예시일 뿐 특별 대우 대상이 아니다.**
//!
//! ```kdl
//! plugin location="file:~/.config/zellij/plugins/polycanv-launcher.wasm" {
//!     tool_claude   "claude"
//!     tool_codex    "codex"
//!     tool_pwsh     "pwsh -NoLogo"
//!     tool_내도구    "my-cli --flag"
//! }
//! ```
//!
//! 이 크레이트에는 zellij 의존이 없다 — wasm 글루는 `main.rs`/`plugin.rs` 에만 있고,
//! 파싱과 선택 로직은 호스트에서 `cargo test` 로 돈다.

pub mod tools;

pub use tools::{parse_tools, Tool};
