use crate::app::{App, Focusable};
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
        data.iter().enumerate().map(|(index, author)| {
            let author_name = &author.user_profile.info.uname;
            let uid = author.user_profile.info.uid;

            // Check if this author is selected
            let is_selected = app.selected_author.selected() == Some(index);

            ListItem::new(Line::from(vec![
                if is_selected {
                    Span::styled(author_name.clone(), Style::default().fg(Color::Green))
                } else {
                    Span::raw(author_name.clone())
                },
                Span::from(" ").fg(Color::DarkGray),
                Span::raw(format!("(UID: {})", uid)).fg(Color::DarkGray),
            ]))
        }).collect()
    } else {
        vec![ListItem::new("No data available")]
    };

    let authors_focused = app.focused_panel() == Focusable::MomentsAuthors;
    let is_list_nav = app.input_mode() == crate::app::InputMode::ListNav;

    // Calculate block color based on focus and mode
    let block_color = if authors_focused && is_list_nav {
        Color::Cyan
    } else if authors_focused {
        Color::Green
    } else {
        Color::Reset
    };

    let authors_list = List::new(authors)
        .block(
            ratatui::widgets::Block::default()
                .title("Following Authors")
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(block_color))
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
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
            app.dynamics_viewport_height = viewport_height; // Store for scrolling handler
            let visible_dynamics = dynamics.iter().skip(app.dynamics_scroll_offset).take(viewport_height);

            for (display_index, dynamic) in visible_dynamics.enumerate() {
                let actual_index = app.dynamics_scroll_offset + display_index;
                let is_selected_dynamic = app.selected_dynamic_index == actual_index;
                let is_list_nav = app.input_mode() == crate::app::InputMode::ListNav;

                // Dynamic header with selection indicator and play option if video
                let mut header_parts = vec![
                    if is_selected_dynamic && is_list_nav {
                        ">> ".fg(Color::Cyan)
                    } else {
                        "   ".fg(Color::Reset)
                    },
                    Span::raw(format!("Dynamic #{}", actual_index + 1)).fg(Color::Cyan)
                ];

                // Add [P]lay option for video dynamics when in ListNav mode
                if dynamic.video_info.is_some() && is_selected_dynamic && is_list_nav {
                    header_parts.push(Span::raw(" "));
                    header_parts.push("[P]lay".fg(Color::Green));
                }

                content.push(Line::from(header_parts));
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

    let content_focused = app.focused_panel() == Focusable::MomentsContent;
    let is_list_nav = app.input_mode() == crate::app::InputMode::ListNav;

    // Calculate block color based on focus and mode
    let block_color = if content_focused && is_list_nav {
        Color::Cyan
    } else if content_focused {
        Color::Green
    } else {
        Color::Reset  // Use default color instead of White
    };

    let content_panel = Paragraph::new(selected_author_content)
        .wrap(ratatui::widgets::Wrap { trim: true })
        .block(
            ratatui::widgets::Block::default()
                .title("Author Details & Dynamics")
                .borders(ratatui::widgets::Borders::ALL)
                .border_style(Style::default().fg(block_color))
        );
    f.render_widget(content_panel, moments_chunks[1]);
}