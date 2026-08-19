//! 세션 묶기 — 작업 흐름별 정체성.
//!
//! **세션에 순서만 있고 자리가 없으면 탭 목록과 다를 게 없다.** 패인 3번이 3번인 이유가
//! "세 번째라서"뿐이면, 사용자는 그걸 기억할 수 없다.
//!
//! 그래서 같은 작업에 속한 세션을 **하나의 덩어리로 읽히게** 한다. 개념은
//! [cate](https://github.com/0-AI-UG/cate) 에서 왔다 — worktree 마다 캔버스 위에 자기 색깔의
//! 영역을 줘서, 다섯 브랜치의 다섯 에이전트가 탭 더미가 아니라 다섯 작업 흐름으로 읽히게 하는 것.

use serde::{Deserialize, Serialize};

/// 한 작업 흐름. 지금은 **작업 디렉터리**가 기준이다.
///
/// 왜 cwd 인가 — 사용자가 "그 작업"이라고 말할 때 가리키는 것이 대개 디렉터리이고,
/// 런처가 패인을 띄울 때 이미 알고 있는 값이라 **추가 조회가 없다**.
/// (git worktree 루트가 더 정확하지만 그건 호스트 왕복이 필요하다 — 그 비용은 이미
/// 사이드바 메타데이터 조회에서 데어봤다.)
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GroupKey(pub String);

impl GroupKey {
    /// cwd 에서 그룹을 만든다. cwd 를 모르면 그룹도 없다 —
    /// **모르는 것을 그럴듯하게 채우지 않는다.**
    pub fn from_cwd(cwd: Option<&str>) -> Option<Self> {
        let cwd = cwd?.trim_end_matches('/');
        if cwd.is_empty() {
            return None;
        }
        Some(GroupKey(cwd.to_string()))
    }

    /// 화면에 보일 짧은 이름 — 경로의 마지막 조각.
    ///
    /// 전체 경로는 이미 줄에 따로 표시된다. 여기서 또 길게 쓰면 폭만 잡아먹는다.
    pub fn label(&self) -> &str {
        self.0
            .rsplit('/')
            .next()
            .filter(|s| !s.is_empty())
            .unwrap_or(&self.0)
    }
}

/// 그룹에 배정되는 색. 터미널 기본 16색만 쓴다 —
/// 256색·트루컬러는 터미널마다 다르게 나오고, 사용자 테마와 충돌한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupColor {
    Cyan,
    Magenta,
    Yellow,
    Green,
    Blue,
    Red,
}

impl GroupColor {
    /// 배정 순서. 인접한 색이 헷갈리지 않도록 대비가 큰 순으로 놓았다.
    pub const PALETTE: [GroupColor; 6] = [
        GroupColor::Cyan,
        GroupColor::Magenta,
        GroupColor::Yellow,
        GroupColor::Green,
        GroupColor::Blue,
        GroupColor::Red,
    ];

    /// ANSI 전경색 코드.
    pub fn ansi(self) -> u8 {
        match self {
            GroupColor::Cyan => 36,
            GroupColor::Magenta => 35,
            GroupColor::Yellow => 33,
            GroupColor::Green => 32,
            GroupColor::Blue => 34,
            GroupColor::Red => 31,
        }
    }
}

/// 그룹 키에서 색을 정한다.
///
/// **같은 디렉터리는 언제나 같은 색이어야 한다.** 세션을 껐다 켜도, 순서가 바뀌어도.
/// 그래야 색이 기억의 단서가 된다 — 매번 달라지면 색이 있으나 마나다.
/// 그래서 배정 순서가 아니라 **키의 해시**로 고른다.
pub fn color_for(key: &GroupKey) -> GroupColor {
    // FNV-1a. 암호학적 용도가 아니라 분산만 필요하고, 의존성을 늘리지 않는다.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in key.0.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }

    // ★ 확산(avalanche)이 필요하다. `% 6` 은 하위 비트만 쓰는데, FNV 는 짧고 비슷한
    //   문자열에서 하위 비트가 잘 안 섞인다 — `/w/api` `/w/web` 처럼 접두사가 같은
    //   경로들이 같은 색으로 뭉친다(테스트로 잡았다: 6개 경로가 3색). splitmix64 의
    //   마무리 단계를 빌려 상위 비트를 하위로 내린다.
    hash ^= hash >> 30;
    hash = hash.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    hash ^= hash >> 27;
    hash = hash.wrapping_mul(0x94d0_49bb_1331_11eb);
    hash ^= hash >> 31;

    GroupColor::PALETTE[(hash % GroupColor::PALETTE.len() as u64) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 같은_디렉터리는_언제나_같은_색이다() {
        // 색이 기억의 단서가 되려면 이게 깨지면 안 된다.
        let k = GroupKey::from_cwd(Some("/home/user/work/polycanv")).unwrap();
        let first = color_for(&k);
        for _ in 0..100 {
            assert_eq!(color_for(&k), first);
        }
    }

    #[test]
    fn 끝의_슬래시는_같은_그룹으로_본다() {
        let a = GroupKey::from_cwd(Some("/home/user/work"));
        let b = GroupKey::from_cwd(Some("/home/user/work/"));
        assert_eq!(
            a, b,
            "슬래시 하나로 그룹이 갈리면 사용자는 이유를 알 수 없다"
        );
    }

    #[test]
    fn cwd_를_모르면_그룹도_없다() {
        assert!(GroupKey::from_cwd(None).is_none());
        assert!(GroupKey::from_cwd(Some("")).is_none());
        assert!(GroupKey::from_cwd(Some("/")).is_none());
    }

    #[test]
    fn 라벨은_경로의_마지막_조각이다() {
        assert_eq!(
            GroupKey::from_cwd(Some("/home/user/work/polycanv"))
                .unwrap()
                .label(),
            "polycanv"
        );
        assert_eq!(
            GroupKey::from_cwd(Some("polycanv")).unwrap().label(),
            "polycanv"
        );
    }

    #[test]
    fn 접두사가_같은_경로들이_한_색으로_쏠리지_않는다() {
        // 실제 사용 형태는 `~/work/<프로젝트>` 처럼 **접두사가 같고 끝만 다른** 경로들이다.
        // 해시가 끝부분을 제대로 섞지 못하면 전부 한 색이 되어 색이 의미를 잃는다.
        //
        // 주의: 소수 표본으로 "전부 다른 색"을 요구하면 안 된다. 6칸에 6개를 넣을 때
        // 기대되는 서로 다른 색은 약 4.0 이다(생일 문제) — 3색이 나와도 정상이다.
        // 그래서 표본을 키워 **쏠림**을 본다.
        use std::collections::BTreeMap;
        let mut counts: BTreeMap<GroupColor, usize> = BTreeMap::new();
        let n = 120;
        for i in 0..n {
            let key = GroupKey::from_cwd(Some(&format!("/home/user/work/project-{i}"))).unwrap();
            *counts.entry(color_for(&key)).or_default() += 1;
        }
        assert_eq!(
            counts.len(),
            GroupColor::PALETTE.len(),
            "쓰이지 않는 색이 있다: {counts:?}"
        );

        // 균등하면 각 색 20개. 한 색이 표본의 40%를 넘으면 해시가 한쪽으로 쏠린 것이다.
        let worst = counts.values().max().copied().unwrap_or(0);
        assert!(
            worst * 100 / n <= 40,
            "한 색이 {worst}/{n} 을 가져갔다: {counts:?}"
        );
    }
}
