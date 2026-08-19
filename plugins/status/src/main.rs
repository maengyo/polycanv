//! wasm 진입점.
//!
//! ★ zellij 는 **바이너리 크레이트**를 로드한다. `[lib] crate-type=["cdylib"]` 로 만들면
//!   `_start` 가 없어 **빌드는 성공하고 로드만 실패한다** — 실행해 보기 전까지 드러나지 않는다.
//!   (실측: 이 파일이 없던 동안 상태 플러그인은 한 번도 로드된 적이 없었다.)

// lib 쪽 `register_plugin!` 이 만든 export 를 바이너리에 링크시킨다.
// 이 참조가 없으면 링커가 통째로 버리고, **49KB 짜리 빈 wasm 이 나온다**(실측).
#[cfg(target_arch = "wasm32")]
extern crate polycanv_status;

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "polycanv-status 는 zellij 플러그인이다. wasm32-wasip1 로 빌드해 zellij 에서 로드해라."
    );
}
