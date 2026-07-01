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

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Clear, Paragraph, Tabs, Widget},
};
use tui_term::widget::{Cursor, PseudoTerminal};

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
                .highlight_style(Style::default().fg(Color::White))
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
                Line::from("  Arrows / hjkl  : Select process"),
                Line::from("  1-9            : Quick jump to process"),
                Line::from("  f / Enter      : Toggle fullscreen"),
                Line::from("  PgUp/PgDn      : Scroll selected terminal"),
                Line::from("  u / d          : Scroll selected terminal"),
                Line::from("  End            : Jump to live output"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Actions (on selected):",
                    Style::default().bold().underlined(),
                )]),
                Line::from("  t              : S[t]art process"),
                Line::from("  s              : [s]top process"),
                Line::from("  e              : R[e]start process"),
                Line::from("  i              : [i]nteractive Mode"),
                Line::from(""),
                Line::from(vec![Span::styled(
                    "Interactive Mode:",
                    Style::default().bold().underlined(),
                )]),
                Line::from("  Ctrl-A         : Exit Interactive Mode"),
                Line::from("  Alt+PgUp/PgDn  : Scroll in interactive"),
                Line::from("  Alt+End        : Jump to live output"),
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
            .border_style(Style::default().fg(Color::DarkGray))
            .title_style(Style::default().fg(Color::White));

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
                Span::styled(" r", Style::default().fg(process.color).bold()),
                Span::raw("estart "),
            ];

            let stop_label = vec![
                Span::styled("s", Style::default().fg(process.color).bold()),
                Span::raw("top "),
            ];

            let start_label = vec![
                Span::raw(" star"),
                Span::styled("t ", Style::default().fg(process.color).bold()),
            ];

            let up_label = vec![
                Span::styled("u", Style::default().fg(process.color).bold()),
                Span::raw("p"),
            ];

            let down_label = vec![
                Span::styled("d", Style::default().fg(process.color).bold()),
                Span::raw("own"),
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

            block = block
                .title(Line::from(help_label).right_aligned())
                .title(Line::from(full_screen_label).right_aligned())
                .title_bottom(Line::from(restart_label).right_aligned())
                .title_bottom(Line::from(stop_label).right_aligned())
                .title_bottom(Line::from(start_label).right_aligned())
                .title_bottom(Line::from(interactive_label).left_aligned())
                .title_bottom(Line::from(up_label).left_aligned())
                .title_bottom(Line::from(down_label).left_aligned());
        }

        let inner_area = block.inner(area);
        block.render(area, buf);

        // Render pseudoterminal using tui-term
        let parser = process.parser.read().unwrap();
        let screen = parser.screen();

        let mut cursor = Cursor::default();
        if !is_interactive {
            cursor.hide();
        }

        let pseudo_term = PseudoTerminal::new(screen).cursor(cursor);
        pseudo_term.render(inner_area, buf);
    }
}

fn process_title(process: &crate::process::Process, index: usize, selected: bool) -> Vec<Span<'_>> {
    let status_str = match process.status {
        crate::process::ProcessStatus::Running => "●",
        crate::process::ProcessStatus::Stopped => "○",
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
