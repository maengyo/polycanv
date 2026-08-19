//! wasm 진입점. zellij 는 **바이너리 크레이트**를 로드한다 (cdylib 은 `_start` 가 없다).

#[cfg(target_arch = "wasm32")]
mod plugin;

#[cfg(target_arch = "wasm32")]
fn main() {}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!(
        "polycanv-launcher 는 zellij 플러그인이다. wasm32-wasip1 로 빌드해 zellij 에서 로드해라."
    );
}
