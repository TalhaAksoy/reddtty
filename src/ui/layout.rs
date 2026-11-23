//! Responsive layout math. This module contains no widget rendering so the
//! same rectangles can drive both drawing and hit-testing.

use ratatui::layout::{Constraint, Layout, Rect};

use crate::config::Settings;

pub const FEED_HEADER_HEIGHT: u16 = 3;

#[derive(Clone, Copy, Debug)]
pub struct RootRects {
    pub top: Rect,
    pub body: Rect,
    pub status: Rect,
}

pub fn root(area: Rect) -> RootRects {
    if area.height <= 3 {
        return RootRects {
            top: Rect::new(area.x, area.y, area.width, 0),
            body: area,
            status: Rect::new(area.x, area.y, area.width, 0),
        };
    }
    let [top, body, status] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(area);
    RootRects { top, body, status }
}

#[derive(Clone, Copy, Debug)]
pub struct FeedRects {
    pub sidebar: Option<Rect>,
    pub feed: Rect,
    pub right: Option<Rect>,
}

pub fn feed(body: Rect, settings: &Settings, allow_right: bool) -> FeedRects {
    let mut sidebar = None;
    let mut right = None;
    let mut feed_x = body.x;
    let mut feed_w = body.width;

    let sidebar_enabled = settings.general.show_sidebar && body.width >= 76;
    if sidebar_enabled {
        let width = if body.width >= 110 {
            settings.ui.sidebar_width.clamp(18, 30)
        } else {
            20
        };
        if feed_w > width + 40 {
            sidebar = Some(Rect::new(body.x, body.y, width, body.height));
            feed_x += width + 1;
            feed_w = feed_w.saturating_sub(width + 1);
        }
    }

    let right_enabled = settings.ui.show_right_panel && allow_right && body.width >= 130;
    if right_enabled {
        let width = if body.width >= 170 { 32 } else { 28 };
        if feed_w > width + 46 {
            right = Some(Rect::new(
                body.x + body.width - width,
                body.y,
                width,
                body.height,
            ));
            feed_w = feed_w.saturating_sub(width + 1);
        }
    }

    FeedRects {
        sidebar,
        feed: Rect::new(feed_x, body.y, feed_w, body.height),
        right,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FeedListRects {
    pub header: Rect,
    pub list: Rect,
}

pub fn feed_list(area: Rect) -> FeedListRects {
    let header_h = if area.height < 12 || area.width < 96 {
        area.height.min(2)
    } else {
        FEED_HEADER_HEIGHT
    };
    FeedListRects {
        header: Rect::new(area.x, area.y, area.width, header_h),
        list: Rect::new(
            area.x,
            area.y + header_h,
            area.width,
            area.height.saturating_sub(header_h),
        ),
    }
}

/// Card area and a one-column scrollbar gutter.
pub fn cards_and_scrollbar(list: Rect) -> (Rect, Option<Rect>) {
    if list.width > 12 {
        (
            Rect::new(list.x, list.y, list.width - 1, list.height),
            Some(Rect::new(list.x + list.width - 1, list.y, 1, list.height)),
        )
    } else {
        (list, None)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DetailParams {
    pub title_lines: u16,
    pub has_comments: bool,
    pub has_image: bool,
    pub has_body: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct DetailRects {
    pub post_pane: Rect,
    pub meta: Rect,
    pub title: Rect,
    pub metrics: Rect,
    pub image: Option<Rect>,
    pub body: Rect,
    pub comments_pane: Rect,
    pub comments_header: Rect,
    pub comments_list: Rect,
}

pub fn detail(area: Rect, params: DetailParams) -> DetailRects {
    let min_comments = 7u16;
    let (post_h, comments_h) = if params.has_comments && area.height >= 12 {
        let desired = (area.height as u32 * 50 / 100) as u16;
        let post_h = desired
            .clamp(8, area.height.saturating_sub(min_comments))
            .min(area.height);
        (post_h, area.height.saturating_sub(post_h))
    } else {
        (area.height, 0)
    };

    let post_pane = Rect::new(area.x, area.y, area.width, post_h);
    let pad_x = post_pane.x + 1;
    let content_w = post_pane.width.saturating_sub(2);

    let mut y = post_pane.y;
    let meta = Rect::new(pad_x, y, content_w, 1.min(post_pane.height));
    y = y.saturating_add(1);

    let title_lines = params
        .title_lines
        .max(1)
        .min(post_pane.height.saturating_sub(2).max(1));
    let title = Rect::new(pad_x, y, content_w, title_lines);
    y = y.saturating_add(title_lines + 1);

    let metrics = Rect::new(pad_x, y, content_w, 1.min(post_pane.height));
    y = y.saturating_add(1);

    let mut image = None;
    let mut image_h = 0u16;
    if params.has_image && post_pane.height >= 16 {
        let available = post_pane.bottom().saturating_sub(y + 2);
        image_h = (post_pane.height / 3).clamp(5, 16).min(available);
        if image_h >= 5 {
            image = Some(Rect::new(pad_x, y + 1, content_w, image_h));
            y = y.saturating_add(1 + image_h);
        } else {
            image_h = 0;
        }
    }
    let _ = image_h;

    let body_top = y;
    let body_h = if params.has_body {
        post_pane.bottom().saturating_sub(body_top)
    } else {
        0
    };
    let body = Rect::new(pad_x, body_top, content_w, body_h);

    let comments_pane = Rect::new(area.x, area.y + post_h, area.width, comments_h);
    let comments_header = Rect::new(
        comments_pane.x + 1,
        comments_pane.y,
        comments_pane.width.saturating_sub(2),
        comments_h.min(2),
    );
    let comments_list = Rect::new(
        comments_pane.x + 1,
        comments_pane.y + comments_h.min(2),
        comments_pane.width.saturating_sub(2),
        comments_h.saturating_sub(comments_h.min(2)),
    );

    DetailRects {
        post_pane,
        meta,
        title,
        metrics,
        image,
        body,
        comments_pane,
        comments_header,
        comments_list,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GalleryRects {
    pub header: Rect,
    pub image: Rect,
    pub footer: Rect,
}

pub fn gallery(area: Rect) -> GalleryRects {
    let header = Rect::new(area.x, area.y, area.width, area.height.min(1));
    let footer_h = area.height.min(1);
    let image = Rect::new(
        area.x,
        area.y + header.height,
        area.width,
        area.height.saturating_sub(header.height + footer_h),
    );
    let footer = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(footer_h),
        area.width,
        footer_h,
    );
    GalleryRects {
        header,
        image,
        footer,
    }
}

pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width
        .min(area.width.saturating_sub(2))
        .max(area.width.min(1));
    let height = height
        .min(area.height.saturating_sub(2))
        .max(area.height.min(1));
    Rect::new(
        area.x + (area.width.saturating_sub(width)) / 2,
        area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    #[test]
    fn three_columns_on_wide_terminal() {
        let s = Settings::default();
        let r = feed(Rect::new(0, 1, 160, 40), &s, true);
        assert!(r.sidebar.is_some());
        assert!(r.right.is_some());
        assert!(r.feed.width >= 50);
    }

    #[test]
    fn two_columns_on_medium_terminal() {
        let s = Settings::default();
        let r = feed(Rect::new(0, 1, 100, 30), &s, true);
        assert!(r.sidebar.is_some());
        assert!(r.right.is_none());
    }

    #[test]
    fn single_column_on_narrow_terminal() {
        let s = Settings::default();
        let r = feed(Rect::new(0, 1, 60, 20), &s, true);
        assert!(r.sidebar.is_none());
        assert!(r.right.is_none());
        assert_eq!(r.feed.width, 60);
    }

    #[test]
    fn tiny_terminal_does_not_panic() {
        let _settings = Settings::default();
        let r = root(Rect::new(0, 0, 20, 4));
        assert!(r.body.height > 0);
        let d = detail(
            Rect::new(0, 0, 20, 4),
            DetailParams {
                title_lines: 2,
                has_comments: true,
                has_image: true,
                has_body: true,
            },
        );
        assert!(d.post_pane.height <= 4);
        let _ = feed_list(Rect::new(0, 0, 20, 4));
        let _ = gallery(Rect::new(0, 0, 20, 4));
    }

    #[test]
    fn detail_image_can_be_hidden_for_short_panes() {
        let d = detail(
            Rect::new(0, 0, 80, 20),
            DetailParams {
                title_lines: 2,
                has_comments: true,
                has_image: true,
                has_body: true,
            },
        );
        assert!(d.image.is_none() || d.image.unwrap().height <= 16);
    }

    #[test]
    fn centers_modals() {
        let r = centered(Rect::new(0, 0, 100, 40), 50, 20);
        assert_eq!(r.width, 50);
        assert_eq!(r.height, 20);
        assert_eq!(r.x, 25);
        assert_eq!(r.y, 10);
    }
}
