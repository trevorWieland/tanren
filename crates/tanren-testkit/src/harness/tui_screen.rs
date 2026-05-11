//! Screen-buffer terminal sanitizer for TUI transcript processing.

const SCREEN_ROWS: usize = 40;
const SCREEN_COLS: usize = 120;

pub(super) fn sanitize_terminal_text(raw: &str) -> String {
    let mut screen = vec![vec![' '; SCREEN_COLS]; SCREEN_ROWS];
    let mut cursor = CursorPos::default();
    let mut chars = raw.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if let Some('[') = chars.peek().copied() {
                chars.next();
                let mut params = String::new();
                let mut final_ch = '\0';
                for code in chars.by_ref() {
                    if code.is_ascii_digit() || code == ';' || code == '?' {
                        params.push(code);
                    } else if ('@'..='~').contains(&code) {
                        final_ch = code;
                        break;
                    } else {
                        break;
                    }
                }
                apply_csi(&mut screen, &mut cursor, &params, final_ch);
            }
            continue;
        }
        if ch == '\n' {
            cursor.advance_row();
            continue;
        }
        if ch == '\r' {
            cursor.col = 0;
            continue;
        }
        cursor.put_char(&mut screen, ch);
    }

    render_screen(&screen)
}

#[derive(Default)]
struct CursorPos {
    row: usize,
    col: usize,
}

impl CursorPos {
    fn advance_row(&mut self) {
        self.row = (self.row + 1).min(SCREEN_ROWS - 1);
        self.col = 0;
    }

    fn put_char(&mut self, screen: &mut [Vec<char>], ch: char) {
        if self.row < SCREEN_ROWS && self.col < SCREEN_COLS {
            screen[self.row][self.col] = ch;
            self.col = (self.col + 1).min(SCREEN_COLS - 1);
        }
    }

    fn move_up(&mut self, n: usize) {
        self.row = self.row.saturating_sub(n).min(SCREEN_ROWS - 1);
    }

    fn move_down(&mut self, n: usize) {
        self.row = (self.row + n).min(SCREEN_ROWS - 1);
    }

    fn move_right(&mut self, n: usize) {
        self.col = (self.col + n).min(SCREEN_COLS - 1);
    }

    fn move_left(&mut self, n: usize) {
        self.col = self.col.saturating_sub(n).min(SCREEN_COLS - 1);
    }

    fn set_position(&mut self, r: usize, c: usize) {
        self.row = r.min(SCREEN_ROWS - 1);
        self.col = c.min(SCREEN_COLS - 1);
    }
}

fn apply_csi(screen: &mut [Vec<char>], cursor: &mut CursorPos, params: &str, final_ch: char) {
    match final_ch {
        'H' | 'f' => {
            let parts: Vec<usize> = params
                .trim_start_matches('?')
                .split(';')
                .filter_map(|s| s.parse().ok())
                .collect();
            let r = parts.first().copied().unwrap_or(1).saturating_sub(1);
            let c = parts.get(1).copied().unwrap_or(1).saturating_sub(1);
            cursor.set_position(r, c);
        }
        'A' => cursor.move_up(csi_param(params)),
        'B' => cursor.move_down(csi_param(params)),
        'C' => cursor.move_right(csi_param(params)),
        'D' => cursor.move_left(csi_param(params)),
        'J' => erase_display(screen, cursor.row, cursor.col, params),
        'K' => erase_line(screen, cursor.row, cursor.col, params),
        _ => {}
    }
}

fn csi_param(params: &str) -> usize {
    params
        .trim_start_matches('?')
        .split(';')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
}

fn erase_display(screen: &mut [Vec<char>], row: usize, col: usize, params: &str) {
    let p = params.trim_start_matches('?');
    match p {
        "0" | "" => {
            if col < SCREEN_COLS {
                for c in &mut screen[row][col..] {
                    *c = ' ';
                }
            }
            for r in &mut screen[row.saturating_add(1)..] {
                for c in r.iter_mut() {
                    *c = ' ';
                }
            }
        }
        "2" => {
            for r in screen.iter_mut() {
                for c in r.iter_mut() {
                    *c = ' ';
                }
            }
        }
        _ => {}
    }
}

fn erase_line(screen: &mut [Vec<char>], row: usize, col: usize, params: &str) {
    let p = params.trim_start_matches('?');
    match p {
        "0" | "" => {
            if col < SCREEN_COLS {
                for c in &mut screen[row][col..] {
                    *c = ' ';
                }
            }
        }
        "2" => {
            for c in &mut screen[row] {
                *c = ' ';
            }
        }
        _ => {}
    }
}

fn render_screen(screen: &[Vec<char>]) -> String {
    let mut out = String::with_capacity(SCREEN_ROWS * (SCREEN_COLS + 1));
    for line in screen {
        let trimmed_end = line.iter().rposition(|c| *c != ' ').map_or(0, |i| i + 1);
        if trimmed_end > 0 {
            let line_str: String = line[..trimmed_end].iter().collect();
            out.push_str(&line_str);
            out.push('\n');
        }
    }
    out
}

pub(super) fn normalize_for_match(raw: &str) -> String {
    raw.chars()
        .filter(char::is_ascii_alphanumeric)
        .flat_map(char::to_lowercase)
        .collect()
}
