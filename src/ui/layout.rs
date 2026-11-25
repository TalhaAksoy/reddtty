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
    /// Estimated wrapped body lines used to size the post pane responsively.
    pub body_lines: u16,
    pub has_comments: bool,
    pub has_image: bool,
    pub has_body: bool,
}

/// Post detail with an optional community/about column on wide terminals.
#[derive(Clone, Copy, Debug)]
pub struct PostRects {
    pub main: Rect,
    pub right: Option<Rect>,
}

pub fn post(area: Rect, settings: &Settings) -> PostRects {
    let mut main = area;
    let mut right = None;
    if settings.ui.show_right_panel && area.width >= 130 {
        let width = if area.width >= 170 { 32 } else { 28 };
        if main.width > width + 46 {
            right = Some(Rect::new(
                area.x + area.width - width,
                area.y,
                width,
                area.height,
            ));
            main = Rect::new(area.x, area.y, area.width - width - 1, area.height);
        }
    }
    PostRects { main, right }
}

fn detail_image_height(width: u16) -> u16 {
    (width / 8).clamp(5, 14)
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
        // Size the post pane to its actual content, with a cap so comments stay
        // usable. A short text post no longer wastes half the screen.
        let available_with_comments = area.height.saturating_sub(min_comments).max(1);
        let ratio_cap = ((area.height as u32 * 60) / 100) as u16;
        let max_post = ratio_cap.max(1).min(available_with_comments);
        let header = 3u16.saturating_add(params.title_lines);
        let image_extra = if params.has_image {
            1u16.saturating_add(detail_image_height(area.width))
        } else {
            0
        };
        let body_extra = if params.has_body {
            params.body_lines
        } else {
            0
        };
        let desired = header
            .saturating_add(image_extra)
            .saturating_add(body_extra);
        let min_post = header.min(max_post).max(1);
        let post_h = desired.clamp(min_post, max_post).min(area.height);
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
    if params.has_image && post_pane.height >= 10 {
        let available = post_pane.bottom().saturating_sub(y + 1);
        let image_h = detail_image_height(post_pane.width).min(available);
        if image_h >= 5 {
            image = Some(Rect::new(pad_x, y + 1, content_w, image_h));
            y = y.saturating_add(1 + image_h);
        }
    }

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
    pub thumbs: Option<Rect>,
    pub footer: Rect,
}

pub fn gallery(area: Rect, show_thumbs: bool) -> GalleryRects {
    let header = Rect::new(area.x, area.y, area.width, area.height.min(1));
    let footer_h = area.height.min(1);
    let thumb_h = if show_thumbs && area.height >= 18 && area.width >= 24 {
        5
    } else {
        0
    };
    let image_h = area
        .height
        .saturating_sub(header.height + footer_h + thumb_h);
    let image = Rect::new(area.x, area.y + header.height, area.width, image_h);
    let thumbs = (thumb_h > 0).then(|| {
        Rect::new(
            area.x,
            area.y + header.height + image_h,
            area.width,
            thumb_h,
        )
    });
    let footer = Rect::new(
        area.x,
        area.y + area.height.saturating_sub(footer_h),
        area.width,
        footer_h,
    );
    GalleryRects {
        header,
        image,
        thumbs,
        footer,
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ThumbCell {
    pub index: usize,
    pub area: Rect,
    pub image: Rect,
}

/// Lay out the visible slice of a gallery thumbnail strip. The selected image
/// stays roughly centered; callers use the same geometry for fetching and
/// rendering so cache keys match.
pub fn gallery_thumbnails(area: Rect, count: usize, selected: usize) -> Vec<ThumbCell> {
    if count == 0 || area.height < 5 || area.width < 12 {
        return Vec::new();
    }
    let cell_w = 10u16.min((area.width.saturating_sub(2) / count as u16).max(6));
    let stride = cell_w.saturating_add(1);
    let visible = (area.width.saturating_sub(2) / stride).max(1) as usize;
    let mut start = selected.saturating_sub(visible / 2);
    if start + visible > count {
        start = count.saturating_sub(visible);
    }
    let mut cells = Vec::new();
    for (slot, index) in (start..count).take(visible).enumerate() {
        let x = area.x + 1 + slot as u16 * stride;
        let image = Rect::new(x + 1, area.y + 1, cell_w.saturating_sub(2), 3);
        cells.push(ThumbCell {
            index,
            area: Rect::new(x, area.y, cell_w, area.height),
            image,
        });
    }
    cells
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
                body_lines: 12,
                has_comments: true,
                has_image: true,
                has_body: true,
            },
        );
        assert!(d.post_pane.height <= 4);
        let _ = feed_list(Rect::new(0, 0, 20, 4));
        let _ = gallery(Rect::new(0, 0, 20, 4), true);
    }

    #[test]
    fn detail_image_can_be_hidden_for_short_panes() {
        let d = detail(
            Rect::new(0, 0, 80, 20),
            DetailParams {
                title_lines: 2,
                body_lines: 12,
                has_comments: true,
                has_image: true,
                has_body: true,
            },
        );
        assert!(d.image.is_none() || d.image.unwrap().height <= 16);
    }

    #[test]
    fn detail_sizes_the_post_pane_to_its_content() {
        let short = detail(
            Rect::new(0, 0, 100, 40),
            DetailParams {
                title_lines: 1,
                body_lines: 1,
                has_comments: true,
                has_image: false,
                has_body: true,
            },
        );
        let long = detail(
            Rect::new(0, 0, 100, 40),
            DetailParams {
                title_lines: 1,
                body_lines: 30,
                has_comments: true,
                has_image: false,
                has_body: true,
            },
        );
        assert!(
            short.post_pane.height < long.post_pane.height,
            "short post: {}, long post: {}",
            short.post_pane.height,
            long.post_pane.height
        );
        assert!(short.comments_list.height > long.comments_list.height);
        assert!(
            long.post_pane.height <= 24,
            "post pane should leave comments room"
        );
    }

    #[test]
    fn detail_never_panics_at_the_minimum_comment_height() {
        let d = detail(
            Rect::new(0, 0, 80, 12),
            DetailParams {
                title_lines: 3,
                body_lines: 40,
                has_comments: true,
                has_image: true,
                has_body: true,
            },
        );
        assert!(d.post_pane.height >= 1);
        assert!(d.post_pane.bottom() <= 12);
    }

    #[test]
    fn post_detail_has_an_optional_right_panel() {
        let settings = Settings::default();
        let wide = post(Rect::new(0, 0, 170, 45), &settings);
        assert!(wide.right.is_some());
        assert!(wide.main.width < 170);
        let medium = post(Rect::new(0, 0, 100, 30), &settings);
        assert!(medium.right.is_none());
        assert_eq!(medium.main.width, 100);
    }

    #[test]
    fn gallery_reserves_a_thumbnail_strip_only_when_useful() {
        let wide = gallery(Rect::new(0, 0, 100, 30), true);
        let thumbs = wide.thumbs.expect("thumbnail strip");
        assert!(thumbs.height >= 5);
        assert!(wide.image.height < 30);
        let cells = gallery_thumbnails(thumbs, 12, 6);
        assert!(!cells.is_empty());
        assert!(cells.iter().any(|cell| cell.index == 6));
        let tiny = gallery(Rect::new(0, 0, 100, 14), true);
        assert!(tiny.thumbs.is_none());
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
