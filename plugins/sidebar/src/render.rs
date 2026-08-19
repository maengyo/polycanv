//! 한 줄을 문자열로 만드는 규칙. 폭이 좁아지면 **뒤에서부터** 정보가 사라진다.
//!
//! 사이드바는 캔버스에서 22%, 리스트에서 30% 다. 80칸 터미널이면 각각 17칸·24칸밖에 안 되므로
//! "전부 못 그리면 깨진다"가 아니라 "덜 중요한 것부터 접는다"로 설계한다.
//! 우선순위: 신호등 > 이름 > 도구 > cwd.

use polycanv_protocol::{GroupColor, GroupKey, ViewMode};

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

/// 화면 한 줄이 무엇인가.
///
/// 그룹 머리글이 줄 사이에 끼어들면서 **"화면 N번째 줄 = 목록 N번째 항목"이 더는 성립하지
/// 않는다.** 클릭 좌표를 항목으로 되돌리려면 그린 순서를 그대로 재현해야 하므로,
/// [`screen`] 과 이 함수는 **같은 규칙**을 쓴다 — 한쪽만 고치면 클릭이 어긋난다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Line {
    /// 맨 위 요약 줄.
    Header,
    /// 그룹 머리글. 클릭해도 아무 일도 일어나지 않는다.
    GroupHeader,
    /// 목록 항목. 값은 `rows` 의 인덱스.
    Row(usize),
}

/// 화면에 그려질 줄들을 순서대로 나열한다. [`screen`] 과 클릭 판정이 공유하는 유일한 진실.
pub fn line_map(rows: &[Row], offset: usize, height: usize) -> Vec<Line> {
    let mut lines = vec![Line::Header];
    if rows.is_empty() {
        return lines;
    }
    let body = height.saturating_sub(HEADER_ROWS);
    for (i, row) in rows.iter().enumerate().skip(offset).take(body) {
        if row.starts_group && lines.len() + 1 < height && row.group.is_some() {
            lines.push(Line::GroupHeader);
        }
        if lines.len() >= height {
            break;
        }
        lines.push(Line::Row(i));
    }
    lines
}

/// 클릭한 화면 줄 번호 → 목록 인덱스. 머리글이나 빈 줄을 클릭하면 `None`.
pub fn row_index_at_line(line: usize, rows: &[Row], offset: usize, height: usize) -> Option<usize> {
    match line_map(rows, offset, height).get(line)? {
        Line::Row(i) => Some(*i),
        _ => None,
    }
}

/// 그룹 머리글. 같은 작업의 세션 묶음 위에 한 줄 얹는다.
///
/// **이게 "자리"를 만든다.** 줄 번호는 순서일 뿐이지만, 머리글 아래 묶인 줄들은
/// 사용자가 "그 프로젝트 쪽"이라고 가리킬 수 있는 덩어리가 된다.
pub fn group_line(key: &GroupKey, color: GroupColor, cols: usize) -> String {
    let label = fit(key.label(), cols.saturating_sub(4).max(1));
    let rule_len = cols.saturating_sub(width(&label) + 3);
    format!(
        "\u{1b}[{}m▸ {}{}\u{1b}[0m",
        color.ansi(),
        label,
        " ".repeat(rule_len.min(cols))
    )
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
    // ★ 그리는 순서를 [`line_map`] 에서 받는다. 클릭 판정이 같은 함수를 쓰므로
    //   여기서만 규칙을 바꾸는 실수가 생기지 않는다.
    for line in line_map(rows, offset, height).into_iter().skip(1) {
        match line {
            Line::Header => {}
            Line::GroupHeader => {
                // line_map 이 머리글을 넣었다면 바로 다음 Row 가 그 그룹의 첫 줄이다.
                if let Some(row) = rows.get(next_row_after(rows, offset, out.len(), height)) {
                    if let (Some(g), Some(c)) = (row.group.as_ref(), row.color) {
                        out.push(group_line(g, c, cols));
                    }
                }
            }
            Line::Row(i) => {
                if let Some(row) = rows.get(i) {
                    out.push(colorize(row_line(row, i, cols, i == selected), row.color));
                }
            }
        }
    }
    out
}

/// 지금 위치 다음에 올 항목의 인덱스. 머리글을 그릴 때 어느 그룹인지 알아내는 데 쓴다.
fn next_row_after(rows: &[Row], offset: usize, drawn: usize, height: usize) -> usize {
    line_map(rows, offset, height)
        .into_iter()
        .skip(drawn + 1)
        .find_map(|l| match l {
            Line::Row(i) => Some(i),
            _ => None,
        })
        .unwrap_or(offset)
        .min(rows.len().saturating_sub(1))
}

/// 줄에 그룹 색을 입힌다. 색이 없으면 그대로 둔다 — **색을 지어내지 않는다.**
///
/// 선택 하이라이트(반전)는 이미 줄 안에 들어 있으므로 바깥에서 감싸도 덮이지 않는다.
fn colorize(line: String, color: Option<GroupColor>) -> String {
    match color {
        Some(c) => format!("\u{1b}[{}m{}\u{1b}[0m", c.ansi(), line),
        None => line,
    }
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
            group: polycanv_protocol::GroupKey::from_cwd(cwd),
            color: polycanv_protocol::GroupKey::from_cwd(cwd)
                .as_ref()
                .map(polycanv_protocol::color_for),
            starts_group: false,
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
        let rows: Vec<Row> = (1..=5).map(|i| row(i, "p", None, None)).collect();
        assert_eq!(row_index_at_line(0, &rows, 0, 20), None);
        assert_eq!(row_index_at_line(1, &rows, 0, 20), Some(0));
        assert_eq!(row_index_at_line(1, &rows, 3, 20), Some(3), "스크롤된 상태");
        assert_eq!(row_index_at_line(9, &rows, 0, 20), None, "목록 끝 아래");
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

    #[test]
    fn 그룹_머리글이_끼어도_클릭이_밀리지_않는다() {
        // codex 리뷰가 잡은 버그다. 머리글을 화면에 넣으면서 클릭 좌표 계산을 안 고치면
        // 머리글 아래부터 한 칸씩 밀려 **엉뚱한 세션이 열린다.**
        let mut rows = vec![
            row(1, "a1", None, Some("/w/api")),
            row(2, "a2", None, Some("/w/api")),
            row(3, "b1", None, Some("/w/web")),
        ];
        rows[0].starts_group = true;
        rows[2].starts_group = true;

        // 화면: 0=요약, 1=[api], 2=a1, 3=a2, 4=[web], 5=b1
        assert_eq!(row_index_at_line(0, &rows, 0, 20), None, "요약 줄");
        assert_eq!(
            row_index_at_line(1, &rows, 0, 20),
            None,
            "그룹 머리글은 항목이 아니다"
        );
        assert_eq!(row_index_at_line(2, &rows, 0, 20), Some(0));
        assert_eq!(row_index_at_line(3, &rows, 0, 20), Some(1));
        assert_eq!(
            row_index_at_line(4, &rows, 0, 20),
            None,
            "두 번째 그룹 머리글"
        );
        assert_eq!(row_index_at_line(5, &rows, 0, 20), Some(2));
        assert_eq!(row_index_at_line(6, &rows, 0, 20), None, "빈 줄");
    }

    #[test]
    fn 그린_줄과_클릭_판정이_같은_수만큼_나온다() {
        // 둘이 어긋나면 클릭이 조용히 빗나간다. 규칙이 한 곳에 있는지 확인한다.
        let mut rows = vec![
            row(1, "a", None, Some("/w/api")),
            row(2, "b", None, Some("/w/web")),
            row(3, "c", None, Some("/w/web")),
        ];
        rows[0].starts_group = true;
        rows[1].starts_group = true;

        for height in 3..12 {
            let drawn = screen(&rows, ViewMode::Canvas, 0, 0, height, 40).len();
            let mapped = line_map(&rows, 0, height).len();
            assert_eq!(
                drawn, mapped,
                "height={height} 에서 그린 줄과 판정이 다르다"
            );
        }
    }

    #[test]
    fn 좁은_화면에서는_머리글을_생략한다() {
        // 세션 줄이 밀려나면서까지 머리글을 넣으면 목록을 못 본다.
        let mut rows = vec![row(1, "a", None, Some("/w/api"))];
        rows[0].starts_group = true;
        let lines = line_map(&rows, 0, 2);
        assert!(
            lines.iter().all(|l| !matches!(l, Line::GroupHeader)),
            "높이 2 에서는 요약 + 항목 하나가 전부여야 한다: {lines:?}"
        );
    }
}
