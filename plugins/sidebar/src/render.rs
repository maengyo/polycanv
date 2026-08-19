//! 한 줄을 문자열로 만드는 규칙. 폭이 좁아지면 **뒤에서부터** 정보가 사라진다.
//!
//! 사이드바는 캔버스에서 22%, 리스트에서 30% 다. 80칸 터미널이면 각각 17칸·24칸밖에 안 되므로
//! "전부 못 그리면 깨진다"가 아니라 "덜 중요한 것부터 접는다"로 설계한다.
//! 우선순위: 신호등 > 이름 > 도구 > cwd.

use polycanv_protocol::ViewMode;

use crate::model::Row;

/// 목록 위에 붙는 머리글 줄 수. 클릭 좌표를 줄 번호로 바꿀 때 이만큼 빼야 한다.
pub const HEADER_ROWS: usize = 1;

/// `숫자 + 메인표시 + 신호등 + 공백` = 5칸.
const PREFIX_COLS: usize = 5;

const SELECT_ON: &str = "\u{1b}[7m";
const SELECT_OFF: &str = "\u{1b}[0m";

/// 대략적인 표시 폭. 이모지·CJK 는 2칸으로 센다.
///
/// `unicode-width` 를 끌어오지 않는다 — 신호등 4종과 한글/한자 경로만 맞으면 충분하고,
/// 의존성은 얇게 유지하는 편이 이 크레이트의 방침이다.
pub fn char_width(c: char) -> usize {
    let c = c as u32;
    let wide = matches!(c,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x26AA..=0x26AB
            | 0x1F300..=0x1FAFF);
    if wide {
        2
    } else {
        1
    }
}

pub fn width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}

/// `max` 칸을 넘으면 잘라내고 `…` 를 붙인다.
pub fn fit(s: &str, max: usize) -> String {
    if width(s) <= max {
        return s.to_string();
    }
    if max == 0 {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in s.chars() {
        let w = char_width(c);
        if used + w + 1 > max {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
}

/// 경로는 **뒤쪽이 중요하다.** 앞을 잘라 `…/parent/dir` 로 만든다.
pub fn shorten_path(path: &str, max: usize) -> String {
    if width(path) <= max {
        return path.to_string();
    }
    let mut acc = String::new();
    for part in path.split('/').filter(|s| !s.is_empty()).rev() {
        let cand = format!("/{part}{acc}");
        if width(&cand) + 1 > max {
            break;
        }
        acc = cand;
    }
    if acc.is_empty() {
        fit(path, max)
    } else {
        format!("…{acc}")
    }
}

fn pad(s: &str, cols: usize) -> String {
    let w = width(s);
    if w >= cols {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(cols - w))
    }
}

/// 목록 한 줄. `index` 는 0-기반이고 화면에는 1~9 로 보인다(숫자키와 같은 값).
pub fn row_line(row: &Row, index: usize, cols: usize, selected: bool) -> String {
    let key = if index < 9 {
        char::from_digit(index as u32 + 1, 10).unwrap_or(' ')
    } else {
        ' '
    };
    // 지금 메인 슬롯에 올라와 있는 패인. 사용자가 "지금 보고 있는 게 목록의 어느 줄인지" 알아야 한다.
    let marker = if row.is_main { '▸' } else { ' ' };

    let mut line = String::new();
    line.push(key);
    line.push(marker);
    line.push(row.state.glyph());
    line.push(' ');

    let budget = cols.saturating_sub(PREFIX_COLS);
    let name = fit(&row.name, budget.min((budget * 45 / 100).max(4)));
    let mut rest = budget - width(&name);
    line.push_str(&name);

    if let Some(tool) = row.tool.as_ref() {
        if rest >= 5 {
            let label = fit(tool.label(), (rest - 2).min(9));
            line.push_str("  ");
            line.push_str(&label);
            rest -= 2 + width(&label);
        }
    }

    if let Some(cwd) = row.cwd.as_deref() {
        if rest >= 6 {
            let short = shorten_path(cwd, rest - 2);
            line.push_str("  ");
            line.push_str(&short);
        }
    }

    let line = pad(&line, cols);
    if selected {
        format!("{SELECT_ON}{line}{SELECT_OFF}")
    } else {
        line
    }
}

/// 머리글. 지금 어느 뷰인지와 몇 개인지.
pub fn header_line(mode: ViewMode, count: usize, cols: usize) -> String {
    let label = match mode {
        ViewMode::Canvas => "캔버스",
        ViewMode::List => "리스트",
    };
    let text = format!("─ {label} {count} ");
    let w = width(&text);
    if w >= cols {
        fit(&text, cols)
    } else {
        format!("{text}{}", "─".repeat(cols - w))
    }
}

/// 선택된 줄이 항상 보이도록 창을 민다. 반환은 `(첫 줄 인덱스, 보이는 줄 수)`.
pub fn visible_window(len: usize, selected: usize, height: usize) -> (usize, usize) {
    if len == 0 || height == 0 {
        return (0, 0);
    }
    let count = height.min(len);
    let offset = if selected < count {
        0
    } else {
        (selected + 1).saturating_sub(count)
    };
    (offset.min(len - count), count)
}

/// 클릭한 화면 줄 번호 → 목록 인덱스. 머리글이나 빈 줄을 클릭하면 `None`.
pub fn row_index_at_line(line: usize, len: usize, offset: usize) -> Option<usize> {
    let index = offset + line.checked_sub(HEADER_ROWS)?;
    (index < len).then_some(index)
}

/// 화면 전체. `offset` 은 [`visible_window`] 가 준 값이어야 클릭 좌표와 어긋나지 않는다.
pub fn screen(
    rows: &[Row],
    mode: ViewMode,
    selected: usize,
    offset: usize,
    height: usize,
    cols: usize,
) -> Vec<String> {
    let mut out = vec![header_line(mode, rows.len(), cols)];
    if rows.is_empty() {
        out.push(fit("터미널이 없다", cols));
        return out;
    }
    let body = height.saturating_sub(HEADER_ROWS);
    for (i, row) in rows.iter().enumerate().skip(offset).take(body) {
        out.push(row_line(row, i, cols, i == selected));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use polycanv_protocol::{AgentState, PaneKey, ToolKind};

    fn row(id: u32, name: &str, tool: Option<ToolKind>, cwd: Option<&str>) -> Row {
        Row {
            key: PaneKey::Terminal(id),
            name: name.to_string(),
            tool,
            cwd: cwd.map(|s| s.to_string()),
            state: AgentState::Idle,
            is_main: false,
            is_suppressed: false,
        }
    }

    #[test]
    fn 신호등은_두칸이다() {
        assert_eq!(char_width('⚪'), 2);
        assert_eq!(char_width('🔴'), 2);
        assert_eq!(char_width('a'), 1);
    }

    #[test]
    fn 좁아도_폭을_넘지_않는다() {
        let r = row(
            1,
            "claude-메인작업",
            Some(ToolKind::ClaudeCode),
            Some("/Users/k/very/deep/path"),
        );
        for cols in 6..80 {
            let line = row_line(&r, 0, cols, false);
            let plain = line.replace(SELECT_ON, "").replace(SELECT_OFF, "");
            assert_eq!(
                width(&plain),
                cols,
                "cols={cols} 에서 폭이 어긋났다: {plain:?}"
            );
        }
    }

    #[test]
    fn 좁아지면_cwd_부터_사라지고_신호등은_남는다() {
        let r = row(
            1,
            "claude",
            Some(ToolKind::ClaudeCode),
            Some("/Users/k/proj"),
        );
        let wide = row_line(&r, 0, 60, false);
        assert!(wide.contains("/Users/k/proj"), "{wide}");
        assert!(wide.contains("claude"));

        let narrow = row_line(&r, 0, 12, false);
        assert!(!narrow.contains("/Users"), "{narrow}");
        assert!(narrow.contains('⚪'), "{narrow}");
    }

    #[test]
    fn 경로는_뒤쪽을_남긴다() {
        assert_eq!(shorten_path("/a/bb/ccc/dddd", 40), "/a/bb/ccc/dddd");
        let s = shorten_path("/home/user/work/polycanv", 16);
        assert!(s.ends_with("/polycanv"), "{s}");
        assert!(s.starts_with('…'), "{s}");
        assert!(width(&s) <= 16, "{s}");
    }

    #[test]
    fn 숫자키_표시는_9번까지다() {
        let r = row(1, "x", None, None);
        assert!(row_line(&r, 8, 20, false).starts_with('9'));
        assert!(row_line(&r, 9, 20, false).starts_with(' '));
    }

    #[test]
    fn 선택된_줄만_반전된다() {
        let r = row(1, "x", None, None);
        assert!(row_line(&r, 0, 20, true).starts_with(SELECT_ON));
        assert!(!row_line(&r, 0, 20, false).contains(SELECT_ON));
    }

    #[test]
    fn 패인이_많으면_선택된_줄이_보이도록_스크롤한다() {
        // 8개 중 8번째를 골랐는데 3줄만 보이면 5,6,7 번째가 보여야 한다.
        assert_eq!(visible_window(8, 7, 3), (5, 3));
        assert_eq!(visible_window(8, 0, 3), (0, 3));
        assert_eq!(visible_window(2, 1, 9), (0, 2));
    }

    #[test]
    fn 머리글을_클릭하면_아무것도_고르지_않는다() {
        assert_eq!(row_index_at_line(0, 5, 0), None);
        assert_eq!(row_index_at_line(1, 5, 0), Some(0));
        assert_eq!(row_index_at_line(1, 5, 3), Some(3));
        assert_eq!(row_index_at_line(9, 5, 0), None);
    }

    #[test]
    fn 패인이_하나여도_여섯개여도_화면이_넘치지_않는다() {
        for n in [0usize, 1, 6, 12] {
            let rows: Vec<_> = (0..n).map(|i| row(i as u32, "t", None, None)).collect();
            let lines = screen(&rows, ViewMode::List, 0, 0, 5, 24);
            // 머리글 1줄 + 본문은 화면 높이를 넘지 않는다. 목록이 비면 안내 1줄뿐이다.
            let expected = if n == 0 { 2 } else { (1 + n).min(5) };
            assert_eq!(lines.len(), expected, "n={n}");
            for l in &lines {
                let plain = l.replace(SELECT_ON, "").replace(SELECT_OFF, "");
                assert!(width(&plain) <= 24, "n={n} {plain:?}");
            }
        }
    }
}
