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

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(0)])
            .split(area);

        let titles: Vec<Line> = self
            .processes
            .iter()
            .enumerate()
            .map(|(i, p)| Line::from(process_title(p, i, self.selected_index == i)))
            .collect();

        let process = self
            .processes
            .iter()
            .enumerate()
            .find(|(i, _p)| &self.selected_index == i)
            .unwrap()
            .1;

        let help_label = vec![
            Span::raw(" hel"),
            Span::styled("p", Style::default().fg(process.color).bold()),
        ];

        let quit_label = vec![
            Span::styled("q", Style::default().fg(process.color).bold()),
            Span::raw("uit"),
        ];

        Tabs::new(titles)
            .block(
                Block::bordered()
                    .title(" Processes ")
                    .title(Line::from(help_label).right_aligned())
                    .title(Line::from(quit_label).right_aligned())
                    .border_type(BorderType::Rounded),
            )
            .select(self.selected_index)
            .highlight_style(Style::default().fg(Color::White))
            .render(chunks[0], buf);

        if let Some(process) = self.processes.get(self.selected_index) {
            self.render_process(process, true, chunks[1], buf, self.selected_index);
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
                Line::from("  Arrows / hjkl  : Select process tab"),
                Line::from("  1-9            : Quick jump to process"),
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
                Line::from("  r              : [r]estart process"),
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

        if let Some(states) = &self.shutdown_states {
            let modal_area = centered_rect(70, 50, area);
            Clear.render(modal_area, buf);

            let mut lines = vec![
                Line::from(vec![Span::styled(
                    "Shutting down processes...",
                    Style::default().bold(),
                )]),
                Line::from(""),
            ];

            for (idx, process) in self.processes.iter().enumerate() {
                let symbol = match states
                    .get(idx)
                    .copied()
                    .unwrap_or(crate::app::ShutdownStatus::Pending)
                {
                    crate::app::ShutdownStatus::Pending => "○",
                    crate::app::ShutdownStatus::Stopping => "◐",
                    crate::app::ShutdownStatus::Done => "●",
                };

                lines.push(Line::from(format!(" {} {}", symbol, process.name)));
            }

            Paragraph::new(lines)
                .block(
                    Block::bordered()
                        .title(" Closing ")
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .render(modal_area, buf);
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

            let mem_label = vec![
                Span::raw(" Memory: "),
                Span::styled(
                    process.mem.to_string(),
                    Style::default().fg(process.color).bold(),
                ),
                Span::raw(" MB"),
            ];

            let cpu_label = vec![
                Span::raw(" CPU: "),
                Span::styled(
                    process.cpu.to_string(),
                    Style::default().fg(process.color).bold(),
                ),
                Span::raw(" %"),
            ];

            let pid_label = vec![
                Span::raw(" PID: "),
                Span::styled(
                    process.process_id.unwrap_or(0).to_string(),
                    Style::default().fg(process.color).bold(),
                ),
                Span::raw(" "),
            ];

            block = block
                .title_bottom(Line::from(restart_label).right_aligned())
                .title_bottom(Line::from(stop_label).right_aligned())
                .title_bottom(Line::from(start_label).right_aligned())
                .title_bottom(Line::from(interactive_label).left_aligned())
                .title_bottom(Line::from(up_label).left_aligned())
                .title_bottom(Line::from(down_label).left_aligned())
                .title(Line::from(pid_label).right_aligned())
                .title(Line::from(mem_label).right_aligned())
                .title(Line::from(cpu_label).right_aligned());
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
            format!("{} {}: {}", status_str, process.name, process.command),
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
