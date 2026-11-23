use ratatui::layout::Rect;

use crate::models::{FeedSort, FeedSpec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pane {
    Sidebar,
    FeedList,
    PostBody,
    Comments,
    Gallery,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HitTarget {
    Pane(Pane),
    Sidebar(FeedSpec),
    FeedPost(usize),
    FeedSort(FeedSort),
    CommentSort,
    OpenImage,
    CommentRow(usize),
    ModalClose,
    ModalBody,
    ModalItem(usize),
    ModalRangeItem(usize),
    ModalScope,
    ModalSubmit,
    GalleryPrev,
    GalleryNext,
    GalleryOpen,
    Retry,
}

#[derive(Clone, Debug)]
pub struct HitArea {
    pub rect: Rect,
    pub target: HitTarget,
}

#[derive(Clone, Debug, Default)]
pub struct HitMap {
    areas: Vec<HitArea>,
}

impl HitMap {
    pub fn clear(&mut self) {
        self.areas.clear();
    }

    pub fn push(&mut self, rect: Rect, target: HitTarget) {
        if rect.width > 0 && rect.height > 0 {
            self.areas.push(HitArea { rect, target });
        }
    }

    /// Topmost hit. Later-registered areas win, which lets modal items sit
    /// above the modal backdrop.
    pub fn hit_at(&self, x: u16, y: u16) -> Option<&HitTarget> {
        self.areas
            .iter()
            .rev()
            .find(|area| contains(area.rect, x, y))
            .map(|area| &area.target)
    }

    pub fn pane_at(&self, x: u16, y: u16) -> Option<Pane> {
        self.areas.iter().rev().find_map(|area| match area.target {
            HitTarget::Pane(pane) if contains(area.rect, x, y) => Some(pane),
            _ => None,
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = &HitArea> {
        self.areas.iter()
    }
}

fn contains(rect: Rect, x: u16, y: u16) -> bool {
    x >= rect.x
        && x < rect.x.saturating_add(rect.width)
        && y >= rect.y
        && y < rect.y.saturating_add(rect.height)
}
