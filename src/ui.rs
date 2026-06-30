//! This program is free software: you can redistribute it and/or modify
//! it under the terms of the GNU General Public License as published by
//! the Free Software Foundation, either version 3 of the License, or
//! (at your option) any later version.
//!
//! This program is distributed in the hope that it will be useful,
//! but WITHOUT ANY WARRANTY; without even the implied warranty of
//! MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//! GNU General Public License for more details.
//!
//! You should have received a copy of the GNU General Public License
//! along with this program.  If not, see <https://www.gnu.org/licenses/>.

use ansi_to_tui::IntoText;
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, BorderType, Clear, Paragraph, Tabs, Widget, Wrap},
};

use crate::app::App;

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.processes.is_empty() {
            Paragraph::new("No processes found. Check your Procfile.")
                .centered()
                .render(area, buf);
            return;
        }

        let body_area = area;

        if let Some(idx) = self.fullscreen_index {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(0)])
                .split(body_area);

            let titles: Vec<Line> = self
                .processes
                .iter()
                .enumerate()
                .map(|(i, p)| Line::from(process_title(p, i, idx == i)))
                .collect();

            Tabs::new(titles)
                .block(
                    Block::bordered()
                        .title(" Processes ")
                        .border_type(BorderType::Rounded),
                )
                .select(idx)
                .highlight_style(Style::default().fg(ratatui::style::Color::White))
                .render(chunks[0], buf);

            if let Some(process) = self.processes.get(idx) {
                self.render_process(process, true, chunks[1], buf, idx);
            }
        } else {
            let num_processes = self.processes.len();
            let num_cols = 2;
            let num_rows = num_processes.div_ceil(num_cols);

            let vertical_constraints = vec![Constraint::Ratio(1, num_rows as u32); num_rows];
            let rows = Layout::default()
                .direction(Direction::Vertical)
                .constraints(vertical_constraints)
                .split(body_area);

            for (i, row_area) in rows.iter().enumerate() {
                let horizontal_constraints = vec![Constraint::Ratio(1, num_cols as u32); num_cols];
                let cols = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(horizontal_constraints)
                    .split(*row_area);

                for (j, col_area) in cols.iter().enumerate() {
                    let process_idx = i * num_cols + j;
                    if let Some(process) = self.processes.get(process_idx) {
                        let is_selected = process_idx == self.selected_index;
                        self.render_process(process, is_selected, *col_area, buf, process_idx);
                    }
                }
            }
        }

        if self.show_help {
            let help_area = centered_rect(60, 60, area);
            let block = Block::bordered()
                .title(" Help ")
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Cyan));

            let help_lines = vec![
                Line::from(vec![Span::styled(
                    "Navigation:",
                    Style::default().bold().underlined(),
                )]),
                Line::from("  Arrows / hjkl  : Select process / Scroll logs"),
                Line::from("  1-9            : Quick jump to process"),
                Line::from("  f / Enter      : Toggle fullscreen"),
                Line::from("  gg / G         : Jump to top/bottom"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Actions (on selected):",
                    Style::default().bold().underlined(),
                )]),
                Line::from("  t              : S[t]art process"),
                Line::from("  s              : [s]top process"),
                Line::from("  e              : R[e]start process"),
                Line::from("  i              : [i]nteractive Mode"),
                Line::from("  a              : Se[a]rch logs (highlights)"),
                Line::from("  r              : Filte[r] logs (hides lines)"),
                Line::from("  w              : Toggle line [w]rap"),
                Line::from("  Delete         : Clear Search/Filter"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Search Navigation:",
                    Style::default().bold().underlined(),
                )]),
                Line::from("  n / N          : Next / Prev Match"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Interactive Mode:",
                    Style::default().bold().underlined(),
                )]),
                Line::from("  Ctrl-A         : Exit Interactive Mode"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Global:",
                    Style::default().bold().underlined(),
                )]),
                Line::from("  p / ?          : Show/Hide this help"),
                Line::from("  q / Ctrl-C     : Quit"),
            ];

            buf.set_style(help_area, Style::default().bg(Color::Black));
            Clear.render(help_area, buf);

            Paragraph::new(help_lines)
                .block(block)
                .render(help_area, buf);
        }
    }
}

impl App {
    fn render_process(
        &self,
        process: &crate::process::Process,
        is_selected: bool,
        area: Rect,
        buf: &mut Buffer,
        index: usize,
    ) {
        let is_interactive =
            is_selected && matches!(self.input_mode, crate::app::InputMode::Interactive);

        let mut block = Block::bordered()
            .border_type(BorderType::Rounded)
            .title(process_title(process, index, is_selected))
            .border_style(Style::default().fg(ratatui::style::Color::DarkGray))
            .title_style(Style::default().fg(ratatui::style::Color::White));

        if is_selected {
            block = block.border_style(Style::default().fg(process.color));

            let help_label = vec![
                Span::raw(" hel"),
                Span::styled("p", Style::default().fg(process.color).bold()),
            ];

            let full_screen_label = vec![
                Span::styled("f", Style::default().fg(process.color).bold()),
                Span::raw("ullscreen "),
            ];

            let restart_label = vec![
                Span::raw(" r"),
                Span::styled("e", Style::default().fg(process.color).bold()),
                Span::raw("start "),
            ];

            let stop_label = vec![
                Span::styled("s", Style::default().fg(process.color).bold()),
                Span::raw("top "),
            ];

            let start_label = vec![
                Span::raw(" star"),
                Span::styled("t ", Style::default().fg(process.color).bold()),
            ];

            let interactive_label = if is_interactive {
                vec![
                    Span::raw(" exit "),
                    Span::styled("Ctrl-A ", Style::default().fg(process.color).bold()),
                ]
            } else {
                vec![
                    Span::raw(" "),
                    Span::styled("i", Style::default().fg(process.color).bold()),
                    Span::raw("nteractive "),
                ]
            };

            let search_label =
                build_search_label(process, self.input_mode.clone(), self.input_buffer.clone());

            let filter_label =
                build_filter_label(process, self.input_mode.clone(), self.input_buffer.clone());

            block = block
                .title(Line::from(help_label).right_aligned())
                .title(Line::from(full_screen_label).right_aligned())
                .title_bottom(Line::from(restart_label).right_aligned())
                .title_bottom(Line::from(stop_label).right_aligned())
                .title_bottom(Line::from(start_label).right_aligned())
                .title_bottom(Line::from(interactive_label).left_aligned())
                .title_bottom(Line::from(search_label).left_aligned())
                .title_bottom(Line::from(filter_label).left_aligned());
        }

        let inner_area = block.inner(area);
        block.render(area, buf);

        if is_interactive {
            let vt = process.vt.lock().unwrap();
            let screen = vt.screen();

            for row in 0..inner_area.height {
                for col in 0..inner_area.width {
                    let x = inner_area.x + col;
                    let y = inner_area.y + row;
                    if x >= area.right() || y >= area.bottom() {
                        continue;
                    }

                    if let Some(cell_to_set) = buf.cell_mut((x, y)) {
                        if let Some(cell) = screen.cell(row, col) {
                            let mut style = Style::default();
                            match cell.fgcolor() {
                                vt100::Color::Rgb(r, g, b) => style = style.fg(Color::Rgb(r, g, b)),
                                vt100::Color::Idx(i) => style = style.fg(Color::Indexed(i)),
                                _ => {}
                            }
                            match cell.bgcolor() {
                                vt100::Color::Rgb(r, g, b) => style = style.bg(Color::Rgb(r, g, b)),
                                vt100::Color::Idx(i) => style = style.bg(Color::Indexed(i)),
                                _ => {}
                            }
                            if cell.bold() {
                                style = style.add_modifier(Modifier::BOLD);
                            }
                            if cell.italic() {
                                style = style.add_modifier(Modifier::ITALIC);
                            }
                            if cell.underline() {
                                style = style.add_modifier(Modifier::UNDERLINED);
                            }

                            let mut symbol = cell.contents();
                            if symbol.is_empty() {
                                symbol = " ".to_string();
                            }
                            cell_to_set.set_symbol(&symbol).set_style(style);
                        } else {
                            // Clear cell if vt100 has no data for it
                            cell_to_set.set_symbol(" ").set_style(Style::default());
                        }
                    }
                }
            }

            // Draw cursor
            let (cursor_row, cursor_col) = screen.cursor_position();
            let cursor_x = inner_area.x + cursor_col;
            let cursor_y = inner_area.y + cursor_row;
            if cursor_x < area.right()
                && cursor_y < area.bottom()
                && let Some(cell) = buf.cell_mut((cursor_x, cursor_y))
            {
                cell.set_style(Style::default().bg(Color::White).fg(Color::Black));
            }
        } else {
            let lines = process.output.lock().unwrap();
            let mut log_text = Text::default();
            for (i, line) in lines.iter().enumerate() {
                let line_str = line.as_str();

                if let Some(filter) = &process.filter
                    && !line_str.to_lowercase().contains(&filter.to_lowercase())
                {
                    continue;
                }

                // Highlighting Search Query
                let rendered_line = if let Some(query) = &process.search_query {
                    let mut spans = Vec::new();
                    let line_lower = line_str.to_lowercase();
                    let query_lower = query.to_lowercase();
                    let mut last_pos = 0;

                    let is_active_line = Some(i) == process.active_match_line;

                    for (start, end) in find_all_matches(&line_lower, &query_lower) {
                        if start > last_pos {
                            spans.push(Span::raw(line_str[last_pos..start].to_string()));
                        }

                        let match_style = if is_active_line {
                            Style::default().bg(Color::Green).fg(Color::Black).bold()
                        } else {
                            Style::default().bg(Color::Yellow).fg(Color::Black).bold()
                        };

                        spans.push(Span::styled(line_str[start..end].to_string(), match_style));
                        last_pos = end;
                    }
                    if last_pos < line_str.len() {
                        spans.push(Span::raw(line_str[last_pos..].to_string()));
                    }
                    Line::from(spans)
                } else {
                    // Try to render ANSI, fallback to raw
                    match line_str.as_bytes().into_text() {
                        Ok(ansi) => {
                            let mut lines = ansi.lines.into_iter();
                            if let Some(l) = lines.next() {
                                l
                            } else {
                                Line::from("")
                            }
                        }
                        Err(_) => Line::from(line_str),
                    }
                };

                log_text.lines.push(rendered_line);
            }

            let height = inner_area.height as usize;

            if self.wrap {
                // Only wrap the tail that can fill the viewport: each logical line wraps to
                // >=1 row, so the last `height + scroll` lines always cover the window. Feeding
                // the whole buffer would re-wrap up to 1000 lines every frame (30 FPS).
                let scroll = process.scroll as usize;
                let n = log_text.lines.len();
                let start = n.saturating_sub(height + scroll + 1);
                let tail = Text::from(log_text.lines[start..].to_vec());

                let paragraph = Paragraph::new(tail).wrap(Wrap { trim: false });
                let total_rows = paragraph.line_count(inner_area.width);
                let current_scroll = total_rows
                    .saturating_sub(height)
                    .saturating_sub(scroll)
                    .min(u16::MAX as usize) as u16;

                paragraph
                    .scroll((current_scroll, 0))
                    .render(inner_area, buf);
            } else {
                let max_scroll = log_text.lines.len().saturating_sub(height) as u16;

                // If scroll is 0, we auto-scroll to the bottom.
                let current_scroll = if process.scroll == 0 {
                    max_scroll
                } else {
                    max_scroll.saturating_sub(process.scroll)
                };

                Paragraph::new(log_text)
                    .scroll((current_scroll, 0))
                    .render(inner_area, buf);
            }
        }
    }
}

fn process_title(process: &crate::process::Process, index: usize, selected: bool) -> Vec<Span<'_>> {
    let status_str = match process.status {
        crate::process::ProcessStatus::Running => "●",
        crate::process::ProcessStatus::Stopped => "○",
        crate::process::ProcessStatus::Failed => "×",
    };

    let superscripts = [" ¹", " ²", " ³", " ⁴", " ⁵", " ⁶", " ⁷", " ⁸", " ⁹"];
    let idx_str = if index < 9 { superscripts[index] } else { "" };

    let color = if selected {
        Color::White
    } else {
        Color::DarkGray
    };

    vec![
        Span::styled(
            idx_str,
            Style::default()
                .fg(process.color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} {}: {} ", status_str, process.name, process.command),
            Style::default().fg(color),
        ),
    ]
}

fn find_all_matches(line: &str, query: &str) -> Vec<(usize, usize)> {
    let mut matches = Vec::new();
    if query.is_empty() {
        return matches;
    }
    let mut start = 0;
    while let Some(pos) = line[start..].find(query) {
        let actual_pos = start + pos;
        matches.push((actual_pos, actual_pos + query.len()));
        start = actual_pos + query.len();
    }
    matches
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn build_search_label(
    process: &crate::process::Process,
    input_mode: crate::app::InputMode,
    input_buffer: String,
) -> Vec<Span<'_>> {
    if input_mode != crate::app::InputMode::Insert(crate::app::InsertField::Search) {
        if let Some(q) = &process.search_query {
            vec![
                Span::raw(" "),
                Span::styled(q, Style::default().fg(Color::Yellow).bold()),
                Span::styled(" del ", Style::default().fg(process.color).bold()),
            ]
        } else {
            vec![
                Span::raw(" se"),
                Span::styled("a", Style::default().fg(process.color).bold()),
                Span::raw("rch "),
            ]
        }
    } else {
        vec![
            Span::raw(" "),
            Span::styled(input_buffer, Style::default().fg(Color::Yellow)),
            Span::styled("█", Style::default().fg(Color::White)),
        ]
    }
}

fn build_filter_label(
    process: &crate::process::Process,
    input_mode: crate::app::InputMode,
    input_buffer: String,
) -> Vec<Span<'_>> {
    if input_mode != crate::app::InputMode::Insert(crate::app::InsertField::Filter) {
        if let Some(f) = &process.filter {
            vec![
                Span::raw(" "),
                Span::styled(f, Style::default().fg(Color::Yellow).bold()),
                Span::styled(" del ", Style::default().fg(process.color).bold()),
            ]
        } else {
            vec![
                Span::raw(" filte"),
                Span::styled("r", Style::default().fg(process.color).bold()),
                Span::raw(" "),
            ]
        }
    } else {
        vec![
            Span::raw(" "),
            Span::styled(input_buffer, Style::default().fg(Color::Yellow)),
            Span::styled("█", Style::default().fg(Color::White)),
        ]
    }
}
