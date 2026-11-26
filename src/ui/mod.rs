pub mod layout;
pub mod screens;
pub mod theme;
pub mod widgets;

use ratatui::Frame;
use ratatui::widgets::Block;

use crate::app::{App, View};
use crate::ui::theme::Theme;

pub const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub fn spinner_char(index: usize) -> &'static str {
    SPINNER[index % SPINNER.len()]
}

pub fn render(frame: &mut Frame, app: &mut App) {
    let theme: Theme = app.theme();
    if frame.area().width < 16 || frame.area().height < 5 {
        frame.render_widget(Block::default().style(theme.base()), frame.area());
        frame.render_widget(
            ratatui::widgets::Paragraph::new("Terminal too small — resize to at least 16×5.")
                .style(theme.base())
                .alignment(ratatui::layout::Alignment::Center),
            frame.area(),
        );
        return;
    }
    let root = layout::root(frame.area());
    frame.render_widget(Block::default().style(theme.base()), frame.area());
    app.hit_map.clear();

    let spinner = spinner_char(app.spinner);
    let spinner_index = app.spinner;

    let title = app.breadcrumb();
    let loading = match app.nav.last() {
        Some(View::Feed(feed)) => feed.loading_initial || feed.loading_more || feed.refreshing,
        Some(View::Post(post)) => post.post_loading || post.comments_loading,
        _ => false,
    };

    if root.top.height > 0 {
        widgets::top_bar::render(
            frame,
            root.top,
            &title,
            &app.provider_label,
            app.demo,
            app.provider_status
                .filter(|status| *status != crate::providers::ProviderStatus::Available)
                .map(|status| status.label()),
            loading,
            spinner,
            &theme,
        );
    }

    {
        let App {
            nav,
            settings,
            focus,
            images,
            sidebar,
            provider,
            demo,
            revealed,
            library,
            hit_map,
            ..
        } = app;
        match nav.last_mut() {
            Some(View::Feed(feed)) => {
                screens::feed::render(
                    frame,
                    root.body,
                    feed,
                    settings,
                    *focus,
                    images,
                    sidebar,
                    provider.as_ref(),
                    *demo,
                    spinner,
                    spinner_index,
                    revealed,
                    library,
                    &theme,
                    hit_map,
                );
            }
            Some(View::Post(post)) => {
                screens::post::render(
                    frame,
                    root.body,
                    post,
                    settings,
                    *focus,
                    images,
                    spinner,
                    spinner_index,
                    revealed,
                    &library.read_comments,
                    &theme,
                    hit_map,
                );
            }
            Some(View::Gallery(gallery)) => {
                screens::gallery::render(
                    frame,
                    root.body,
                    gallery,
                    settings,
                    images,
                    spinner_index,
                    &theme,
                    hit_map,
                );
            }
            None => {}
        }
    }

    if root.status.height > 0 {
        widgets::status_bar::render(
            frame,
            root.status,
            app.nav.last(),
            app.focus,
            app.modal.as_ref(),
            &theme,
        );
    }

    if let Some(modal) = &app.modal {
        // Keep the top bar and status hints, but remove the noisy feed behind
        // the modal. Search/help/sort then read as a single focused surface.
        frame.render_widget(ratatui::widgets::Clear, root.body);
        frame.render_widget(
            ratatui::widgets::Block::default().style(theme.base()),
            root.body,
        );
        screens::overlays::render_modal(
            frame,
            frame.area(),
            modal,
            &app.library,
            &app.settings,
            &app.health,
            &app.provider_label,
            app.demo,
            &app.images,
            app.config_corrupt,
            &theme,
            &mut app.hit_map,
        );
    }

    // Notifications must be the last visual layer: modal actions such as
    // "Test now", filter changes and image-protocol switches need visible
    // success/error feedback.
    let notification_area = if app.modal.is_some() {
        // Keep modal footer controls visible: modal feedback shares the
        // status-bar row instead of painting over the modal body.
        root.status
    } else {
        match app.nav.last() {
            Some(View::Feed(_)) => layout::feed(root.body, &app.settings, true).feed,
            _ => root.body,
        }
    };
    screens::overlays::render_notifications(frame, notification_area, &app.notifications, &theme);
}
