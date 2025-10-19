use crate::app::{App, Focusable};
use crate::ui::traits::WidgetRenderer;
use ratatui::prelude::*;
use ratatui::widgets::{List, ListItem, Paragraph};

pub fn render_moments_panel(f: &mut Frame, area: Rect, app: &mut App) {
    // Split the main content area into two columns for moments
    let moments_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(area);

    // Left panel: Authors list
    let authors: Vec<ListItem> = if let Some(data) = &app.moments_data {
        data.iter().map(|author| {
            let author_name = &author.user_profile.info.uname;
            let uid = author.user_profile.info.uid;

            ListItem::new(Line::from(vec![
                Span::raw(author_name.clone()),
                Span::from(" ").fg(Color::DarkGray),
                Span::raw(format!("(UID: {})", uid)).fg(Color::DarkGray),
            ]))
        }).collect()
    } else {
        vec![ListItem::new("No data available")]
    };

    let authors_focused = app.focused_panel == Focusable::MomentsAuthors;
    let authors_list = List::new(authors)
        .block(app.create_focused_block("Following Authors", authors_focused))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Green))
        .highlight_symbol(">> ");
    f.render_stateful_widget(authors_list, moments_chunks[0], &mut app.selected_author);

    // Right panel: Selected author dynamics
    let selected_author_content = if app.loading_dynamics {
        vec![
            Line::from("Loading dynamics...".italic().fg(Color::Yellow)),
            Line::from(""),
            Line::from("Please wait while fetching author's latest updates.".fg(Color::DarkGray)),
        ]
    } else if let Some(dynamics) = &app.selected_author_dynamics {
        if dynamics.is_empty() {
            vec![
                Line::from("No dynamics available".fg(Color::DarkGray)),
                Line::from(""),
                Line::from("This author hasn't posted any recent updates.".fg(Color::DarkGray)),
            ]
        } else {
            let mut content = vec![];
            content.push(Line::from("Author Dynamics".bold()));
            content.push(Line::from(""));

            // Show dynamics starting from scroll offset, calculate viewport height
            let viewport_height = moments_chunks[1].height.saturating_sub(2) as usize; // Subtract border lines
            let visible_dynamics = dynamics.iter().skip(app.dynamics_scroll_offset).take(viewport_height);

            for (display_index, dynamic) in visible_dynamics.enumerate() {
                let actual_index = app.dynamics_scroll_offset + display_index;
                content.push(Line::from(format!("Dynamic #{}", actual_index + 1).fg(Color::Cyan)));
                content.push(Line::from(""));

                // Author info
                content.push(Line::from(vec![
                    "Author: ".bold(),
                    Span::raw(dynamic.author_name.clone()).fg(Color::Green),
                ]));

                // Timestamp
                if dynamic.timestamp > 0 {
                    let datetime = chrono::DateTime::from_timestamp(dynamic.timestamp as i64, 0)
                        .unwrap_or_default();
                    content.push(Line::from(vec![
                        "Time: ".bold(),
                        Span::raw(datetime.format("%Y-%m-%d %H:%M").to_string()).fg(Color::White),
                    ]));
                } else {
                    content.push(Line::from(vec![
                        "Time: ".bold(),
                        Span::raw("Unknown").fg(Color::White),
                    ]));
                }
                content.push(Line::from(""));

                // Content
                if !dynamic.content.is_empty() {
                    let content_text = if dynamic.content.chars().count() > 200 {
                        let truncated: String = dynamic.content.chars().take(200).collect();
                        format!("{}...", truncated)
                    } else {
                        dynamic.content.clone()
                    };
                    content.push(Line::from("Content:".bold()));
                    content.push(Line::from(Span::raw(content_text)));
                    content.push(Line::from(""));
                }

                // Video info if available
                if let Some(video) = &dynamic.video_info {
                    content.push(Line::from("📹 Video:".bold()));
                    content.push(Line::from(vec![
                        "Title: ".italic(),
                        Span::raw(video.title.clone()),
                    ]));
                    content.push(Line::from(vec![
                        "Duration: ".italic(),
                        Span::raw(video.duration_text.clone()).fg(Color::White),
                    ]));
                    content.push(Line::from(vec![
                        "Plays: ".italic(),
                        Span::raw(video.stat.play.clone()).fg(Color::White),
                    ]));
                    content.push(Line::from(""));
                }

                // Stats if available
                if let Some(stats) = &dynamic.stats {
                    content.push(Line::from("Stats:".bold()));
                    content.push(Line::from(vec![
                        "👍 ".fg(Color::Green),
                        Span::raw(format!(" {}", stats.like.count)),
                        "  💬 ".fg(Color::Blue),
                        Span::raw(format!(" {}", stats.comment.count)),
                        "  🔄 ".fg(Color::Yellow),
                        Span::raw(format!(" {}", stats.forward.count)),
                    ]));
                }

                content.push(Line::from(""));
                content.push(Line::from("─".repeat(50).fg(Color::DarkGray)));
                content.push(Line::from(""));
            }

            // Show scroll position indicator if there are more dynamics
            if dynamics.len() > viewport_height {
                let remaining = dynamics.len() - app.dynamics_scroll_offset - viewport_height.min(dynamics.len() - app.dynamics_scroll_offset);
                if remaining > 0 {
                    content.push(Line::from(format!("... {} more below, {} total", remaining, dynamics.len()).fg(Color::DarkGray)));
                } else {
                    content.push(Line::from(format!("End of {} dynamics", dynamics.len()).fg(Color::DarkGray)));
                }
            }

            content
        }
    } else if let (Some(data), Some(selected_index)) = (&app.moments_data, app.selected_author.selected()) {
        if let Some(author_item) = data.get(selected_index) {
            let author = &author_item.user_profile.info;
            vec![
                Line::from("Select to load dynamics".fg(Color::Yellow)),
                Line::from(""),
                Line::from(vec!["Author: ".bold(), Span::raw(author.uname.clone())]),
                Line::from(vec!["UID: ".bold(), Span::raw(author.uid.to_string())]),
                Line::from(""),
                Line::from("Press Enter or navigate to load this author's dynamics".fg(Color::DarkGray)),
            ]
        } else {
            vec![Line::from("No author selected")]
        }
    } else {
        vec![Line::from("No data available")]
    };

    let content_focused = app.focused_panel == Focusable::MomentsContent;
    let content_panel = Paragraph::new(selected_author_content)
        .wrap(ratatui::widgets::Wrap { trim: true })
        .block(app.create_focused_block("Author Details & Dynamics", content_focused));
    f.render_widget(content_panel, moments_chunks[1]);
}