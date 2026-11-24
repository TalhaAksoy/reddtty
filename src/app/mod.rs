pub mod feed;
pub mod gallery;
pub mod health;
pub mod help;
pub mod hit;
pub mod modal;
pub mod post;
pub mod sidebar;

use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{
    KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Rect;
use tokio::sync::mpsc::UnboundedSender;

use crate::config::Settings;
use crate::error::ProviderError;
use crate::event::{AppEvent, ViewId};
use crate::library::Library;
use crate::media::renderer::ImageManager;
use crate::models::{
    Comment, CommentSort, FeedPage, FeedRequest, FeedSort, FeedSpec, Post, Subreddit, TopRange,
};
use crate::providers::ContentProvider;
use crate::ui::layout;
use crate::ui::theme::Theme;
use crate::ui::widgets::post_card;
use crate::utils::browser;
use crate::utils::text::{normalize_subreddit, sanitize};

use feed::FeedView;
use gallery::GalleryView;
use health::SourceHealth;
use help::HelpModal;
use hit::{HitMap, HitTarget, Pane};
pub use modal::Modal;
use modal::{
    BookmarksModal, CommandId, CommandPaletteModal, ConfigPickerModal, ConfirmAction, FiltersModal,
    RangeModal, SettingsModal, SortChoice, SortContext, SortModal, SourceModal, TextKind,
    TextModal, ThemePickerModal,
};
use post::{PostView, ToggleOutcome};
use sidebar::{SidebarEntry, SidebarState};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    Sidebar,
    Feed,
    PostBody,
    Comments,
}

#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)] // The nav stack is tiny; boxing complicates every access.
pub enum View {
    Feed(FeedView),
    Post(PostView),
    Gallery(GalleryView),
}

impl View {
    pub fn id(&self) -> ViewId {
        match self {
            Self::Feed(v) => v.id,
            Self::Post(v) => v.id,
            Self::Gallery(v) => v.id,
        }
    }

    pub fn title(&self) -> String {
        match self {
            Self::Feed(v) => v.spec.title(),
            Self::Post(v) => format!("Post · r/{}", v.post.subreddit),
            Self::Gallery(_) => "Image Viewer".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Debug)]
pub struct Notification {
    pub text: String,
    pub kind: NoticeKind,
    pub at: Instant,
}

impl Notification {
    fn lifetime(&self) -> Duration {
        match self.kind {
            NoticeKind::Error => Duration::from_secs(9),
            NoticeKind::Warning => Duration::from_secs(7),
            _ => Duration::from_secs(4),
        }
    }

    fn expired(&self, now: Instant) -> bool {
        now.duration_since(self.at) > self.lifetime()
    }
}

pub struct App {
    pub provider: Arc<dyn ContentProvider>,
    pub settings: Settings,
    pub config_path: PathBuf,
    pub tx: UnboundedSender<AppEvent>,
    pub nav: Vec<View>,
    pub focus: Focus,
    pub sidebar: SidebarState,
    pub modal: Option<Modal>,
    pub notifications: VecDeque<Notification>,
    pub images: ImageManager,
    pub spinner: usize,
    pub should_quit: bool,
    pub size: Rect,
    pub hit_map: HitMap,
    pub force_clear: bool,
    pub last_click: Option<(Instant, HitTarget)>,
    pub revealed: HashSet<String>,
    pub next_view_id: u64,
    pub next_request_id: u64,
    pub dirty: bool,
    pub demo: bool,
    /// Short badge shown in the top bar (`DEMO`, `REDDIT`, `OLD REDDIT`, ...).
    pub provider_label: String,
    /// Last provider health observed by the UI. `None` means no request has
    /// completed yet in this session.
    pub provider_status: Option<crate::providers::ProviderStatus>,
    pub persist_error_shown: bool,
    /// Values from the config file, before CLI session overrides. These are
    /// what gets written back unless the user changes them in-app.
    pub persist_theme: crate::config::ThemeMode,
    pub persist_images_enabled: bool,
    pub persist_images_protocol: crate::config::ImageProtocolSetting,
    /// Local bookmarks, read state and filters. Never sent to a provider.
    pub library: Library,
    pub library_path: PathBuf,
    /// In-memory source health shown on the Source screen.
    pub health: SourceHealth,
    /// Start time for each in-flight request, used for latency reporting.
    pub request_starts: HashMap<u64, Instant>,
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider: Arc<dyn ContentProvider>,
        settings: Settings,
        config_path: PathBuf,
        images: ImageManager,
        tx: UnboundedSender<AppEvent>,
        initial_spec: Option<FeedSpec>,
        initial_sort: Option<FeedSort>,
        mut notices: Vec<String>,
    ) -> Self {
        let demo = provider.is_demo();
        let provider_label = provider.display_name();
        let spec = initial_spec.unwrap_or_else(|| default_spec(&settings));
        let requested_sort =
            initial_sort.or_else(|| FeedSort::from_str_loose(&settings.general.default_sort));
        let mut sort = requested_sort.unwrap_or_else(|| spec.default_sort());
        let options = provider.supported_sorts(&spec);
        let sort_supported = options.contains(&sort);
        if !sort_supported {
            sort = options.first().copied().unwrap_or(spec.default_sort());
        }
        let view_id = ViewId(1);
        let mut feed = FeedView::new(view_id, spec.clone(), sort, TopRange::Week);
        feed.loading_initial = true;
        let persist_theme = settings.general.theme;
        let persist_images_enabled = settings.images.enabled;
        let persist_images_protocol = settings.images.protocol;
        let library_path = Library::path_for(&config_path);
        let (library, library_warnings) = Library::load(&library_path);
        notices.extend(library_warnings);

        let mut app = Self {
            provider,
            settings,
            config_path,
            tx,
            nav: vec![View::Feed(feed)],
            focus: Focus::Feed,
            sidebar: SidebarState::default(),
            modal: None,
            notifications: VecDeque::new(),
            images,
            spinner: 0,
            should_quit: false,
            size: Rect::new(0, 0, 100, 30),
            hit_map: HitMap::default(),
            force_clear: false,
            last_click: None,
            revealed: HashSet::new(),
            next_view_id: 2,
            next_request_id: 1,
            dirty: true,
            demo,
            provider_label,
            provider_status: None,
            persist_error_shown: false,
            persist_theme,
            persist_images_enabled,
            persist_images_protocol,
            library,
            library_path,
            health: SourceHealth::default(),
            request_starts: HashMap::new(),
        };
        for notice in notices {
            let kind = if notice.to_ascii_lowercase().contains("demo") {
                NoticeKind::Info
            } else {
                NoticeKind::Warning
            };
            app.notify(kind, notice);
        }
        if let FeedSpec::Subreddit(name) = &spec {
            app.settings.add_recent(name);
        }
        if let Some(requested) = requested_sort
            && !sort_supported
        {
            app.notify(
                NoticeKind::Warning,
                format!(
                    "{} is not available for {}; using {}.",
                    requested.label(),
                    spec.title(),
                    sort.label()
                ),
            );
        }
        app.request_feed_initial(view_id);
        app.request_subreddit_if_needed(view_id);
        app
    }

    // ------------------------------------------------------------------
    // Basic accessors
    // ------------------------------------------------------------------

    pub fn theme(&self) -> Theme {
        Theme::from_mode(self.settings.general.theme)
    }

    pub fn current(&self) -> Option<&View> {
        self.nav.last()
    }

    pub fn current_id(&self) -> Option<ViewId> {
        self.nav.last().map(View::id)
    }

    pub fn current_feed(&self) -> Option<&FeedView> {
        self.nav.last().and_then(|v| match v {
            View::Feed(f) => Some(f),
            _ => None,
        })
    }

    pub fn current_feed_mut(&mut self) -> Option<&mut FeedView> {
        self.nav.last_mut().and_then(|v| match v {
            View::Feed(f) => Some(f),
            _ => None,
        })
    }

    pub fn current_post(&self) -> Option<&PostView> {
        self.nav.last().and_then(|v| match v {
            View::Post(p) => Some(p),
            _ => None,
        })
    }

    pub fn current_post_mut(&mut self) -> Option<&mut PostView> {
        self.nav.last_mut().and_then(|v| match v {
            View::Post(p) => Some(p),
            _ => None,
        })
    }

    pub fn current_gallery(&self) -> Option<&GalleryView> {
        self.nav.last().and_then(|v| match v {
            View::Gallery(g) => Some(g),
            _ => None,
        })
    }

    pub fn current_gallery_mut(&mut self) -> Option<&mut GalleryView> {
        self.nav.last_mut().and_then(|v| match v {
            View::Gallery(g) => Some(g),
            _ => None,
        })
    }

    pub fn feed_mut(&mut self, id: ViewId) -> Option<&mut FeedView> {
        self.nav.iter_mut().find_map(|v| match v {
            View::Feed(f) if f.id == id => Some(f),
            _ => None,
        })
    }

    pub fn post_mut(&mut self, id: ViewId) -> Option<&mut PostView> {
        self.nav.iter_mut().find_map(|v| match v {
            View::Post(p) if p.id == id => Some(p),
            _ => None,
        })
    }

    pub fn active_feed_spec(&self) -> Option<&FeedSpec> {
        self.nav.iter().rev().find_map(|v| match v {
            View::Feed(f) => Some(&f.spec),
            _ => None,
        })
    }

    pub fn active_subreddit(&self) -> Option<String> {
        if let Some(View::Post(p)) = self.nav.last() {
            return Some(p.post.subreddit.clone());
        }
        if let Some(View::Gallery(g)) = self.nav.last() {
            return Some(g.post.subreddit.clone());
        }
        self.active_feed_spec()
            .and_then(|s| s.subreddit_name().map(str::to_string))
    }

    fn alloc_view(&mut self) -> ViewId {
        let id = ViewId(self.next_view_id);
        self.next_view_id += 1;
        id
    }

    fn alloc_request(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id += 1;
        id
    }

    fn begin_request(&mut self) -> u64 {
        let id = self.alloc_request();
        self.request_starts.insert(id, Instant::now());
        id
    }

    /// Record a completed request for the Source screen and the top-bar status.
    fn finish_request(
        &mut self,
        request: u64,
        status: crate::providers::ProviderStatus,
        error: Option<String>,
    ) {
        let latency = self.request_starts.remove(&request).map(|at| at.elapsed());
        self.health.record(status, error, latency);
        self.provider_status = Some(status);
    }

    // ------------------------------------------------------------------
    // Local library
    // ------------------------------------------------------------------

    pub fn save_library(&mut self) {
        if let Err(error) = self.library.save(&self.library_path) {
            self.notify(NoticeKind::Error, error);
        }
    }

    /// Rebuild the provider from the current settings and restart navigation
    /// without leaving the app. Favorites, recents, themes and the local
    /// library are never touched. Old in-flight requests are ignored because
    /// the navigation stack is replaced with new view ids.
    pub fn reload_provider(&mut self) {
        let setup = crate::providers::build_provider(&self.settings, false);
        self.provider = setup.provider;
        self.demo = setup.demo;
        self.provider_label = self.provider.display_name();
        self.provider_status = None;
        self.health.reset();
        self.request_starts.clear();
        self.images.clear_failures();

        let context = self.nav.iter().rev().find_map(|view| match view {
            View::Feed(feed) => Some((feed.spec.clone(), feed.sort, feed.top_range)),
            _ => None,
        });
        let (spec, requested_sort, top_range) = context.unwrap_or_else(|| {
            let sort = FeedSort::from_str_loose(&self.settings.general.default_sort)
                .unwrap_or(FeedSort::Hot);
            (default_spec(&self.settings), sort, TopRange::Week)
        });
        let mut sort = requested_sort;
        let options = self.provider.supported_sorts(&spec);
        if !options.contains(&sort) {
            sort = options.first().copied().unwrap_or(spec.default_sort());
        }

        let id = self.alloc_view();
        let mut feed = FeedView::new(id, spec.clone(), sort, top_range);
        feed.loading_initial = true;
        self.nav.clear();
        self.nav.push(View::Feed(feed));
        self.focus = Focus::Feed;
        self.modal = None;
        self.force_clear = true;
        self.dirty = true;
        self.request_feed_initial(id);
        self.request_subreddit_if_needed(id);

        for notice in setup.notices {
            self.notify(NoticeKind::Warning, notice);
        }
        self.notify(
            NoticeKind::Success,
            format!("Switched source to {}.", self.provider_label),
        );
    }

    // ------------------------------------------------------------------
    // Notifications
    // ------------------------------------------------------------------

    pub fn notify(&mut self, kind: NoticeKind, text: impl Into<String>) {
        let text = sanitize(&text.into());
        let text = crate::utils::text::ellipsize(&text, 240);
        self.notifications.push_back(Notification {
            text,
            kind,
            at: Instant::now(),
        });
        while self.notifications.len() > 5 {
            self.notifications.pop_front();
        }
        self.dirty = true;
    }

    pub fn notify_error(&mut self, error: &ProviderError) {
        self.notify(NoticeKind::Error, error.to_string());
    }

    pub fn animating(&self) -> bool {
        if self.images.animating() || !self.notifications.is_empty() {
            return true;
        }
        match self.nav.last() {
            Some(View::Feed(f)) => {
                f.loading_initial || f.loading_more || f.refreshing || f.request.is_some()
            }
            Some(View::Post(p)) => {
                p.comments_loading || p.post_loading || p.comments_request.is_some()
            }
            _ => false,
        }
    }

    // ------------------------------------------------------------------
    // Navigation
    // ------------------------------------------------------------------

    pub fn open_feed(&mut self, spec: FeedSpec, sort_override: Option<FeedSort>, push: bool) {
        if !push {
            self.nav.clear();
        } else if let Some(View::Feed(current)) = self.nav.last()
            && current.spec == spec
        {
            self.focus = Focus::Feed;
            self.dirty = true;
            return;
        }

        let id = self.alloc_view();
        let mut sort = sort_override.unwrap_or_else(|| spec.default_sort());
        let options = self.provider.supported_sorts(&spec);
        if !options.contains(&sort) {
            sort = options.first().copied().unwrap_or(spec.default_sort());
        }
        let view = FeedView::new(id, spec.clone(), sort, TopRange::Week);
        if let FeedSpec::Subreddit(name) = &spec {
            self.settings.add_recent(name);
            self.persist();
        }
        self.nav.push(View::Feed(view));
        self.modal = None;
        self.focus = Focus::Feed;
        self.force_clear = true;
        self.dirty = true;
        self.request_feed_initial(id);
        self.request_subreddit_if_needed(id);
    }

    pub fn back(&mut self) {
        if self.nav.len() <= 1 {
            self.notify(NoticeKind::Info, "Already at the top level.");
            return;
        }
        self.nav.pop();
        self.force_clear = true;
        self.modal = None;
        self.focus = match self.nav.last() {
            Some(View::Post(_)) => Focus::Comments,
            _ => Focus::Feed,
        };
        if self.library.filters.hide_read {
            self.prune_current_feed();
        }
        self.dirty = true;
    }

    pub fn open_post(&mut self) {
        let post = match self.current_feed().and_then(FeedView::selected_post) {
            Some(post) => post.clone(),
            None => return,
        };
        self.open_post_object(post);
    }

    /// Open a post from any source, including a locally saved bookmark
    /// snapshot. Opening marks the post read and persists that state.
    fn open_post_object(&mut self, post: Post) {
        let id = self.alloc_view();
        let post_removed = post.is_deleted();
        let initial_subreddit = self
            .current_feed()
            .and_then(|feed| feed.subreddit.clone())
            .filter(|subreddit| subreddit.name.eq_ignore_ascii_case(&post.subreddit));
        let mut view = PostView::new(id, post.clone(), CommentSort::Best);
        view.subreddit = initial_subreddit;
        if post_removed {
            view.comments_loading = false;
        }
        if self.library.mark_post_read(&post.id) {
            self.save_library();
        }
        self.nav.push(View::Post(view));
        self.focus = Focus::Comments;
        self.force_clear = true;
        self.dirty = true;
        self.modal = None;
        self.request_post(id);
        if !post_removed {
            self.request_comments(id);
        }
        self.request_post_subreddit(id);
    }

    pub fn open_gallery(&mut self) {
        let post = self.action_post().cloned();
        let Some(post) = post else {
            return;
        };
        if self.is_hidden(&post) {
            self.notify(
                NoticeKind::Warning,
                "Content is hidden. Press x to reveal it first.",
            );
            return;
        }
        let id = self.alloc_view();
        let Some(gallery) = GalleryView::new(id, post) else {
            self.notify(NoticeKind::Info, "This post has no images.");
            return;
        };
        self.nav.push(View::Gallery(gallery));
        self.force_clear = true;
        self.dirty = true;
        self.modal = None;
    }

    fn action_post(&self) -> Option<&Post> {
        match self.nav.last() {
            Some(View::Feed(f)) => f.selected_post(),
            Some(View::Post(p)) => Some(&p.post),
            Some(View::Gallery(g)) => Some(&g.post),
            None => None,
        }
    }

    pub fn open_url(&mut self) {
        let Some(post) = self.action_post().cloned() else {
            return;
        };
        let url = if let Some(View::Gallery(g)) = self.nav.last() {
            g.current().map(|m| m.url.clone())
        } else {
            post.open_url()
        };
        let Some(url) = url else {
            self.notify(NoticeKind::Info, "This post has no external URL.");
            return;
        };
        if url.starts_with("demo://") {
            self.notify(
                NoticeKind::Info,
                "Demo images are local fixtures; there is no external page to open.",
            );
            return;
        }
        if self.demo && matches!(post.content, crate::models::PostContent::Text) {
            self.notify(
                NoticeKind::Info,
                "Demo text posts do not link to a real Reddit thread.",
            );
            return;
        }
        match browser::open_url(&url) {
            Ok(()) => self.notify(NoticeKind::Success, "Opened in your browser."),
            Err(error) => self.notify(NoticeKind::Error, error),
        }
    }

    pub fn reveal_current(&mut self) {
        let Some(post) = self.action_post().cloned() else {
            return;
        };
        let id = post.id.clone();
        let hidden = self.is_hidden(&post);
        if !post.nsfw && !post.spoiler {
            self.notify(NoticeKind::Info, "This post is not marked NSFW or spoiler.");
            return;
        }
        if !hidden {
            self.notify(NoticeKind::Info, "This content is already visible.");
            return;
        }
        self.revealed.insert(id);
        for view in &mut self.nav {
            match view {
                View::Feed(f) => f.invalidate_heights(),
                View::Post(p) => p.invalidate_body(),
                View::Gallery(_) => {}
            }
        }
        self.notify(NoticeKind::Info, "Content revealed for this session.");
    }

    /// The subreddit an action should target: the selected sidebar entry when
    /// the sidebar is focused, otherwise the open post/feed, falling back to
    /// the selected feed card for Home/Popular.
    pub fn current_subreddit_name(&self) -> Option<String> {
        if self.focus == Focus::Sidebar {
            return self
                .sidebar_selected_entry()
                .and_then(|entry| entry.spec.subreddit_name().map(str::to_string));
        }
        self.active_subreddit().or_else(|| {
            self.current_feed()
                .and_then(|feed| feed.selected_post())
                .map(|post| post.subreddit.clone())
        })
    }

    pub fn toggle_favorite(&mut self) {
        let name = self.current_subreddit_name();
        let Some(name) = name else {
            self.notify(
                NoticeKind::Info,
                "Open a subreddit or select a sidebar entry first.",
            );
            return;
        };
        if self.settings.is_favorite(&name) {
            self.modal = Some(Modal::Confirm(
                crate::app::modal::ConfirmModal::remove_favorite(&name),
            ));
            self.dirty = true;
            return;
        }
        let added = self.settings.toggle_favorite(&name);
        self.persist();
        self.clamp_sidebar();
        if added {
            self.notify(NoticeKind::Success, format!("r/{name} added to favorites."));
        }
    }

    fn remove_favorite_now(&mut self, name: &str) {
        if self.settings.is_favorite(name) {
            self.settings.toggle_favorite(name);
            self.persist();
            self.clamp_sidebar();
            self.notify(
                NoticeKind::Info,
                format!("r/{name} removed from favorites."),
            );
        }
        self.dirty = true;
    }

    // ------------------------------------------------------------------
    // Local bookmarks and read state
    // ------------------------------------------------------------------

    pub fn toggle_bookmark_current(&mut self) {
        let Some(post) = self.action_post().cloned() else {
            self.notify(NoticeKind::Info, "Select a post first.");
            return;
        };
        let saved = self.library.toggle_bookmark(&post);
        self.save_library();
        if saved {
            self.notify(
                NoticeKind::Success,
                format!("Saved “{}” with an offline snapshot.", post.title),
            );
        } else {
            self.notify(NoticeKind::Info, "Removed from bookmarks.");
        }
        self.dirty = true;
    }

    pub fn toggle_read_current(&mut self) {
        let Some(post) = self.action_post().cloned() else {
            self.notify(NoticeKind::Info, "Select a post first.");
            return;
        };
        let read = self.library.toggle_post_read(&post.id);
        self.save_library();
        if self.library.filters.hide_read && read {
            self.prune_current_feed();
        }
        self.notify(
            NoticeKind::Info,
            if read {
                "Marked as read."
            } else {
                "Marked as unread."
            },
        );
        self.dirty = true;
    }

    pub fn open_bookmarks(&mut self) {
        self.modal = Some(Modal::Bookmarks(BookmarksModal::default()));
        self.dirty = true;
    }

    fn remove_bookmark_item(&mut self, index: usize) {
        let Some(saved) = self.library.bookmarks.get(index) else {
            return;
        };
        let title = saved.post.title.clone();
        let id = saved.post.id.clone();
        self.library.remove_bookmark(&id);
        self.save_library();
        if let Some(Modal::Bookmarks(modal)) = &mut self.modal {
            modal.clamp(self.library.bookmarks.len());
        }
        self.notify(
            NoticeKind::Info,
            format!("Removed “{title}” from bookmarks."),
        );
        self.dirty = true;
    }

    fn open_bookmark_item(&mut self, index: usize) {
        let Some(post) = self.library.saved_post(index).cloned() else {
            return;
        };
        self.open_post_object(post);
    }

    /// Remove posts hidden by the current local filters from the visible feed.
    /// Reduces selection safely; a refresh restores posts when filters change.
    fn prune_current_feed(&mut self) {
        let App { library, nav, .. } = self;
        if let Some(View::Feed(feed)) = nav.last_mut() {
            let before = feed.posts.len();
            feed.posts
                .retain(|post| !library.filters.hides(post, library.is_post_read(&post.id)));
            if feed.posts.len() != before {
                feed.heights_dirty = true;
                feed.clamp_offset();
                feed.selected = feed.selected.min(feed.posts.len().saturating_sub(1));
            }
        }
        self.request_more_if_needed();
        self.dirty = true;
    }

    pub fn toggle_theme(&mut self) {
        let next = self.settings.general.theme.toggle();
        self.apply_theme_preview(next);
        self.persist_theme = next;
        self.persist();
        self.notify(NoticeKind::Info, format!("Theme: {}", next.label()));
    }

    fn apply_theme_preview(&mut self, theme: crate::config::ThemeMode) {
        if self.settings.general.theme == theme {
            return;
        }
        self.settings.general.theme = theme;
        for view in &mut self.nav {
            if let View::Post(p) = view {
                p.invalidate_body();
            }
        }
        self.dirty = true;
    }

    fn commit_theme(&mut self, theme: crate::config::ThemeMode) {
        self.apply_theme_preview(theme);
        self.persist_theme = theme;
        self.persist();
        self.notify(NoticeKind::Info, format!("Theme: {}", theme.label()));
    }

    pub fn toggle_sidebar(&mut self) {
        self.settings.general.show_sidebar = !self.settings.general.show_sidebar;
        if !self.settings.general.show_sidebar && self.focus == Focus::Sidebar {
            self.focus = Focus::Feed;
        }
        if self.settings.general.show_sidebar && self.size.width < 76 {
            self.notify(
                NoticeKind::Info,
                "The sidebar needs at least 76 columns; it will appear when the terminal is wider.",
            );
        }
        self.persist();
        self.dirty = true;
    }

    pub fn persist(&mut self) {
        let mut to_save = self.settings.clone();
        to_save.general.theme = self.persist_theme;
        to_save.images.enabled = self.persist_images_enabled;
        to_save.images.protocol = self.persist_images_protocol;
        if let Err(error) = to_save.save(&self.config_path) {
            if !self.persist_error_shown {
                self.persist_error_shown = true;
                self.notify(
                    NoticeKind::Error,
                    format!("Could not save settings: {error}"),
                );
            }
            tracing::warn!(error = %error, "failed to save settings");
        }
    }

    // ------------------------------------------------------------------
    // Requests
    // ------------------------------------------------------------------

    fn request_feed_initial(&mut self, view_id: ViewId) {
        let request_id = self.begin_request();
        let limit = self.settings.content.default_post_limit;
        let request = {
            let Some(view) = self.feed_mut(view_id) else {
                return;
            };
            let request = FeedRequest {
                spec: view.spec.clone(),
                sort: view.sort,
                top_range: view.top_range,
                after: None,
                limit,
            };
            let empty = view.posts.is_empty();
            view.request = Some(request_id);
            view.loading_initial = empty;
            view.refreshing = !empty;
            view.loading_more = false;
            view.more_request = None;
            view.error = None;
            if !empty {
                view.pending_selected_id = view.selected_id();
            }
            request
        };
        let provider = self.provider.clone();
        let tx = self.tx.clone();
        self.dirty = true;
        spawn_guarded(async move {
            tracing::debug!(request = request_id, "spawned initial feed request");
            let result = provider.get_feed(request).await;
            tracing::debug!(
                request = request_id,
                ok = result.is_ok(),
                "initial feed request finished"
            );
            let _ = tx.send(AppEvent::FeedLoaded {
                view: view_id,
                request: request_id,
                append: false,
                result,
            });
        });
    }

    fn request_more(&mut self) {
        let request_id = self.begin_request();
        let limit = self.settings.content.default_post_limit;
        let Some(view) = self.current_feed() else {
            return;
        };
        let view_id = view.id;
        let viewport = view.last_viewport;
        if !view.wants_more(viewport) {
            return;
        }
        let Some(after) = view.after.clone() else {
            return;
        };
        let request = FeedRequest {
            spec: view.spec.clone(),
            sort: view.sort,
            top_range: view.top_range,
            after: Some(after),
            limit,
        };
        if let Some(view) = self.feed_mut(view_id) {
            view.more_request = Some(request_id);
            view.loading_more = true;
        }
        let provider = self.provider.clone();
        let tx = self.tx.clone();
        self.dirty = true;
        spawn_guarded(async move {
            let result = provider.get_feed(request).await;
            let _ = tx.send(AppEvent::FeedLoaded {
                view: view_id,
                request: request_id,
                append: true,
                result,
            });
        });
    }

    fn request_more_if_needed(&mut self) {
        let should = self
            .current_feed()
            .is_some_and(|view| view.wants_more(view.last_viewport));
        if should {
            self.request_more();
        }
    }

    fn request_subreddit_if_needed(&mut self, view_id: ViewId) {
        let name = {
            let Some(view) = self.feed_mut(view_id) else {
                return;
            };
            let Some(name) = view.spec.subreddit_name().map(str::to_string) else {
                return;
            };
            if view
                .subreddit
                .as_ref()
                .is_some_and(|s| s.name.eq_ignore_ascii_case(&name))
            {
                return;
            }
            if view.subreddit_request.is_some() {
                return;
            }
            name
        };
        let request_id = self.begin_request();
        let Some(view) = self.feed_mut(view_id) else {
            self.request_starts.remove(&request_id);
            return;
        };
        view.subreddit_request = Some(request_id);
        let provider = self.provider.clone();
        let tx = self.tx.clone();
        spawn_guarded(async move {
            let result = provider.get_subreddit(&name).await;
            let _ = tx.send(AppEvent::SubredditLoaded {
                view: view_id,
                request: request_id,
                name,
                result,
            });
        });
    }

    fn request_post_subreddit(&mut self, view_id: ViewId) {
        let name = {
            let Some(view) = self.post_mut(view_id) else {
                return;
            };
            let name = view.post.subreddit.clone();
            if view
                .subreddit
                .as_ref()
                .is_some_and(|subreddit| subreddit.name.eq_ignore_ascii_case(&name))
                && view.subreddit_error.is_none()
            {
                return;
            }
            if view.subreddit_request.is_some() {
                return;
            }
            name
        };
        let request_id = self.begin_request();
        let Some(view) = self.post_mut(view_id) else {
            self.request_starts.remove(&request_id);
            return;
        };
        view.subreddit_request = Some(request_id);
        let provider = self.provider.clone();
        let tx = self.tx.clone();
        spawn_guarded(async move {
            let result = provider.get_subreddit(&name).await;
            let _ = tx.send(AppEvent::SubredditLoaded {
                view: view_id,
                request: request_id,
                name,
                result,
            });
        });
    }

    fn request_post(&mut self, view_id: ViewId) {
        let request_id = self.begin_request();
        let provider = self.provider.clone();
        let tx = self.tx.clone();
        let post_id = {
            let Some(view) = self.post_mut(view_id) else {
                return;
            };
            view.post_request = Some(request_id);
            view.post_loading = true;
            view.post_error = None;
            view.post.id.clone()
        };
        spawn_guarded(async move {
            let result = provider.get_post(&post_id).await;
            let _ = tx.send(AppEvent::PostLoaded {
                view: view_id,
                request: request_id,
                result,
            });
        });
    }

    fn request_comments(&mut self, view_id: ViewId) {
        let request_id = self.begin_request();
        let provider = self.provider.clone();
        let tx = self.tx.clone();
        let (post_id, sort) = {
            let Some(view) = self.post_mut(view_id) else {
                return;
            };
            view.comments_request = Some(request_id);
            view.mark_comments_loading();
            (view.post.id.clone(), view.comments_sort)
        };
        self.dirty = true;
        spawn_guarded(async move {
            let result = provider.get_comments(&post_id, sort).await;
            let _ = tx.send(AppEvent::CommentsLoaded {
                view: view_id,
                request: request_id,
                result,
            });
        });
    }

    fn request_more_comments(&mut self, view_id: ViewId, more_id: String, children: Vec<String>) {
        let request_id = self.begin_request();
        let provider = self.provider.clone();
        let tx = self.tx.clone();
        let (post_id, sort) = {
            let Some(view) = self.post_mut(view_id) else {
                return;
            };
            let width = view.last_comment_width.max(40);
            view.pending_more.insert(more_id.clone(), request_id);
            view.mark_more_loading(&more_id, true, width);
            (view.post.id.clone(), view.comments_sort)
        };
        self.dirty = true;
        spawn_guarded(async move {
            let result = provider.get_more_comments(&post_id, &children, sort).await;
            let _ = tx.send(AppEvent::MoreCommentsLoaded {
                view: view_id,
                request: request_id,
                more_id,
                result,
            });
        });
    }

    pub fn refresh_current(&mut self) {
        self.images.clear_failures();
        match self.nav.last_mut() {
            Some(View::Feed(_)) => {
                let Some(id) = self.current_id() else {
                    return;
                };
                self.request_feed_initial(id);
                self.request_subreddit_if_needed(id);
                self.notify(NoticeKind::Info, "Refreshing feed…");
            }
            Some(View::Post(_)) => {
                let Some(id) = self.current_id() else {
                    return;
                };
                let removed = self
                    .current_post()
                    .is_some_and(|view| view.post.is_deleted());
                if let Some(view) = self.post_mut(id) {
                    view.post_loading = true;
                    view.post_error = None;
                    if removed {
                        view.comments_loading = false;
                    } else {
                        view.mark_comments_loading();
                    }
                }
                self.request_post(id);
                if !removed {
                    self.request_comments(id);
                }
                self.request_post_subreddit(id);
                self.notify(NoticeKind::Info, "Refreshing post…");
            }
            Some(View::Gallery(_)) => {
                self.force_clear = true;
                self.notify(NoticeKind::Info, "Image refreshed.");
            }
            None => {}
        }
    }

    // ------------------------------------------------------------------
    // Sorting and modals
    // ------------------------------------------------------------------

    pub fn open_sort_modal(&mut self) {
        match self.nav.last() {
            Some(View::Feed(_)) => {
                if let Some(feed) = self.current_feed() {
                    let sorts = self.provider.supported_sorts(&feed.spec);
                    self.modal = Some(Modal::Sort(SortModal::feed(
                        sorts,
                        feed.sort,
                        feed.top_range,
                    )));
                }
            }
            Some(View::Post(p)) => {
                self.modal = Some(Modal::Sort(SortModal::comments(
                    CommentSort::ALL.to_vec(),
                    p.comments_sort,
                )));
            }
            _ => {}
        }
    }

    pub fn open_range_modal(&mut self) {
        let top = self
            .current_feed()
            .map(|f| f.top_range)
            .unwrap_or(TopRange::Week);
        self.modal = Some(Modal::Range(RangeModal::current(top)));
    }

    pub fn open_goto_modal(&mut self) {
        self.modal = Some(Modal::Text(TextModal::goto()));
    }

    pub fn open_search_modal(&mut self) {
        let caps = self.provider.capabilities();
        if !caps.search {
            self.notify(
                NoticeKind::Info,
                "The active provider does not support search.",
            );
            return;
        }
        let current = if caps.search_in_subreddit {
            self.active_subreddit()
        } else {
            None
        };
        self.modal = Some(Modal::Text(TextModal::search(current)));
    }

    pub fn open_help(&mut self) {
        self.modal = Some(Modal::Help(HelpModal::default()));
    }

    pub fn open_command_palette(&mut self) {
        self.modal = Some(Modal::CommandPalette(CommandPaletteModal::default()));
        self.dirty = true;
    }

    fn execute_command(&mut self, command: CommandId) {
        self.modal = None;
        match command {
            CommandId::Help => self.open_help(),
            CommandId::Settings => self.open_settings(),
            CommandId::Filters => self.open_filters(),
            CommandId::SourceHealth => self.open_source_health(),
            CommandId::Bookmarks => self.open_bookmarks(),
            CommandId::Search => self.open_search_modal(),
            CommandId::GoToSubreddit => self.open_goto_modal(),
            CommandId::ChangeSource => {
                self.modal = Some(Modal::ConfigPicker(ConfigPickerModal::default()));
            }
            CommandId::Refresh => self.refresh_current(),
            CommandId::ToggleSidebar => self.toggle_sidebar(),
            CommandId::SortBest => self.apply_feed_sort(FeedSort::Best),
            CommandId::SortHot => self.apply_feed_sort(FeedSort::Hot),
            CommandId::SortNew => self.apply_feed_sort(FeedSort::New),
            CommandId::SortTop => self.apply_feed_sort(FeedSort::Top),
            CommandId::RangeAll => self.apply_top_range(TopRange::All),
            CommandId::ThemeNext => self.toggle_theme(),
            CommandId::ThemeDark => self.commit_theme(crate::config::ThemeMode::Dark),
            CommandId::ThemeLight => self.commit_theme(crate::config::ThemeMode::Light),
            CommandId::ThemeLatte => self.commit_theme(crate::config::ThemeMode::Latte),
            CommandId::ThemeFrappe => self.commit_theme(crate::config::ThemeMode::Frappe),
            CommandId::ThemeMacchiato => self.commit_theme(crate::config::ThemeMode::Macchiato),
            CommandId::ThemeMocha => self.commit_theme(crate::config::ThemeMode::Mocha),
            CommandId::CopyPermalink => self.copy_permalink(),
            CommandId::CopyImageUrl => self.copy_image_url(),
            CommandId::OpenComments => self.open_comments(),
            CommandId::OpenSubreddit => self.open_subreddit(),
            CommandId::MarkRead => self.toggle_read_current(),
            CommandId::AddFilter => {
                self.modal = Some(Modal::Text(TextModal::add_filter()));
            }
        }
    }

    pub fn open_settings(&mut self) {
        self.modal = Some(Modal::Settings(SettingsModal::new(
            self.settings.general.theme,
        )));
    }

    fn activate_settings_item(&mut self, index: usize) {
        match index {
            0 => {
                self.modal = Some(Modal::ConfigPicker(ConfigPickerModal::default()));
            }
            1 => {
                self.modal = Some(Modal::ThemePicker(ThemePickerModal::new(
                    self.settings.general.theme,
                )));
            }
            2 => self.open_source_health(),
            3 => self.open_filters(),
            _ => {}
        }
    }

    pub fn open_filters(&mut self) {
        self.modal = Some(Modal::Filters(FiltersModal::default()));
        self.dirty = true;
    }

    fn filter_row_count(&self) -> usize {
        1 + self.library.filters.entries().len()
    }

    fn toggle_hide_read(&mut self) {
        self.library.filters.hide_read = !self.library.filters.hide_read;
        let enabled = self.library.filters.hide_read;
        self.apply_filter_change(if enabled {
            "Hiding read posts."
        } else {
            "Showing read posts again."
        });
    }

    fn remove_filter_row(&mut self, index: usize) {
        if index == 0 {
            self.notify(NoticeKind::Info, "Press Enter to toggle hide-read.");
            return;
        }
        let Some(entry) = self.library.filters.entries().get(index - 1).cloned() else {
            return;
        };
        self.library.filters.remove_entry(entry.kind, &entry.value);
        let rows = self.filter_row_count().saturating_sub(1);
        if let Some(Modal::Filters(modal)) = &mut self.modal {
            modal.selected = modal.selected.min(rows);
        }
        self.apply_filter_change(format!(
            "Removed {} filter “{}”.",
            entry.kind.label(),
            entry.value
        ));
    }

    fn clear_all_filters(&mut self) {
        if !self.library.filters.is_active() {
            self.notify(NoticeKind::Info, "No local filters are active.");
            return;
        }
        self.library.clear_filters();
        if let Some(Modal::Filters(modal)) = &mut self.modal {
            modal.selected = 0;
        }
        self.apply_filter_change("Cleared all local filters.");
    }

    /// Persist filter changes, prune the visible feed immediately and refetch
    /// the current feed so the active rule set is applied consistently.
    fn apply_filter_change(&mut self, message: impl Into<String>) {
        self.save_library();
        self.prune_current_feed();
        if let Some(View::Feed(feed)) = self.nav.last() {
            let id = feed.id;
            self.request_feed_initial(id);
        }
        self.notify(NoticeKind::Info, message);
        self.dirty = true;
    }

    pub fn open_source_health(&mut self) {
        self.modal = Some(Modal::Source(SourceModal::default()));
        self.dirty = true;
    }

    fn execute_source_action(&mut self, index: usize) {
        match index {
            0 => {
                self.notify(
                    NoticeKind::Info,
                    format!("Testing {}…", self.provider_label),
                );
                self.refresh_current();
                if let Some(Modal::Source(modal)) = &mut self.modal {
                    modal.selected = 0;
                }
            }
            1 => {
                self.modal = Some(Modal::ConfigPicker(ConfigPickerModal::default()));
            }
            _ => {}
        }
    }

    fn activate_config_picker_item(&mut self, index: usize) {
        let Some(preset) = crate::config::presets::PRESETS.get(index) else {
            return;
        };
        let preset_settings: Settings = match toml::from_str(preset.contents) {
            Ok(settings) => settings,
            Err(error) => {
                self.notify(
                    NoticeKind::Error,
                    format!("Could not read bundled config: {error}"),
                );
                return;
            }
        };
        let backup = match crate::config::presets::backup_existing(&self.config_path) {
            Ok(backup) => backup,
            Err(error) => {
                self.notify(NoticeKind::Error, error);
                return;
            }
        };
        // Change the content source only. Favorites, recents, theme, images and
        // UI preferences stay with the user.
        let mut to_save = self.settings.clone();
        to_save.provider = preset_settings.provider;
        to_save.general.theme = self.persist_theme;
        to_save.images.enabled = self.persist_images_enabled;
        to_save.images.protocol = self.persist_images_protocol;
        if let Err(error) = to_save.save(&self.config_path) {
            self.notify(NoticeKind::Error, format!("Could not save config: {error}"));
            return;
        }
        self.settings.provider = to_save.provider;
        let backup = backup
            .map(|path| format!(" Previous config: {}.", path.display()))
            .unwrap_or_default();
        // Apply the new provider immediately; a restart is no longer needed.
        self.reload_provider();
        self.notify(
            NoticeKind::Success,
            format!(
                "Changed to {}. Provider applied immediately.{}",
                preset.name, backup
            ),
        );
    }

    fn apply_feed_sort(&mut self, sort: FeedSort) {
        let Some(view) = self.current_feed() else {
            return;
        };
        let view_id = view.id;
        let current_sort = view.sort;
        let top = view.top_range;
        let spec = view.spec.clone();
        let supported = self.provider.supported_sorts(&spec);
        if !supported.contains(&sort) {
            self.notify(
                NoticeKind::Warning,
                format!("{} is not available for {}.", sort.label(), spec.title()),
            );
            return;
        }
        if current_sort == sort {
            self.modal = None;
            return;
        }
        if let Some(view) = self.feed_mut(view_id) {
            view.reset_for_query(sort, top);
        }
        self.modal = None;
        self.focus = Focus::Feed;
        self.force_clear = true;
        self.request_feed_initial(view_id);
        self.notify(NoticeKind::Info, format!("Sorted by {}.", sort.label()));
    }

    fn apply_top_range(&mut self, range: TopRange) {
        let Some(view) = self.current_feed() else {
            self.modal = None;
            return;
        };
        let view_id = view.id;
        let top_changed = view.top_range != range;
        if let Some(view) = self.feed_mut(view_id) {
            view.reset_for_query(FeedSort::Top, range);
        }
        self.modal = None;
        self.focus = Focus::Feed;
        self.force_clear = true;
        self.request_feed_initial(view_id);
        if top_changed {
            self.notify(NoticeKind::Info, format!("Top · {}", range.label()));
        }
    }

    fn apply_comment_sort(&mut self, sort: CommentSort) {
        let Some(view) = self.current_post() else {
            self.modal = None;
            return;
        };
        let id = view.id;
        if view.comments_sort == sort {
            self.modal = None;
            return;
        }
        if let Some(view) = self.post_mut(id) {
            view.comments_sort = sort;
        }
        self.modal = None;
        self.focus = Focus::Comments;
        self.request_comments(id);
        self.notify(
            NoticeKind::Info,
            format!("Comments sorted by {}.", sort.label()),
        );
    }

    fn submit_text_modal(&mut self) {
        let Some(Modal::Text(modal)) = self.modal.clone() else {
            return;
        };
        match modal.kind {
            TextKind::GoToSubreddit => {
                if modal.value.trim().is_empty() {
                    if let Some(Modal::Text(m)) = &mut self.modal {
                        m.error = Some("Type a subreddit name.".into());
                    }
                    return;
                }
                match normalize_subreddit(&modal.value) {
                    Some(name) => {
                        self.modal = None;
                        self.open_feed(FeedSpec::Subreddit(name), None, true);
                    }
                    None => {
                        if let Some(Modal::Text(m)) = &mut self.modal {
                            m.error =
                                Some("Invalid subreddit name (2–21 letters, digits or _).".into());
                        }
                    }
                }
            }
            TextKind::Search => {
                let query = modal.value.trim().to_string();
                if query.is_empty() {
                    if let Some(Modal::Text(m)) = &mut self.modal {
                        m.error = Some("Type something to search for.".into());
                    }
                    return;
                }
                let scope = modal.search_scope();
                self.modal = None;
                self.open_feed(
                    FeedSpec::Search { query, scope },
                    Some(FeedSort::Best),
                    true,
                );
            }
            TextKind::AddFilter => match self.library.filters.add_expression(&modal.value) {
                Ok(message) => {
                    self.modal = Some(Modal::Filters(FiltersModal { selected: 1 }));
                    self.apply_filter_change(message);
                }
                Err(error) => {
                    if let Some(Modal::Text(m)) = &mut self.modal {
                        m.error = Some(error);
                    }
                }
            },
        }
    }

    fn handle_modal_key(&mut self, key: KeyEvent) {
        let Some(modal) = self.modal.clone() else {
            return;
        };
        if modal_is_text(&modal) {
            match key.code {
                KeyCode::Esc => {
                    if matches!(&modal, Modal::Text(m) if m.kind == TextKind::AddFilter) {
                        self.modal = Some(Modal::Filters(FiltersModal::default()));
                    } else {
                        self.modal = None;
                    }
                    return;
                }
                KeyCode::Enter => {
                    self.submit_text_modal();
                    return;
                }
                KeyCode::Tab => {
                    if let Some(Modal::Text(m)) = &mut self.modal
                        && m.kind == TextKind::Search
                    {
                        m.toggle_scope();
                    }
                    return;
                }
                KeyCode::Backspace => {
                    if let Some(Modal::Text(m)) = &mut self.modal {
                        m.backspace();
                    }
                    return;
                }
                KeyCode::Delete => {
                    if let Some(Modal::Text(m)) = &mut self.modal {
                        m.delete();
                    }
                    return;
                }
                KeyCode::Left => {
                    if let Some(Modal::Text(m)) = &mut self.modal {
                        m.move_cursor(-1);
                    }
                    return;
                }
                KeyCode::Right => {
                    if let Some(Modal::Text(m)) = &mut self.modal {
                        m.move_cursor(1);
                    }
                    return;
                }
                KeyCode::Home => {
                    if let Some(Modal::Text(m)) = &mut self.modal {
                        m.cursor = 0;
                    }
                    return;
                }
                KeyCode::End => {
                    if let Some(Modal::Text(m)) = &mut self.modal {
                        m.cursor = m.value.chars().count();
                    }
                    return;
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    if let Some(Modal::Text(m)) = &mut self.modal {
                        m.clear();
                    }
                    return;
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    if let Some(Modal::Text(m)) = &mut self.modal {
                        m.push_char(c);
                    }
                    return;
                }
                _ => return,
            }
        }

        if let Modal::CommandPalette(mut palette) = modal.clone() {
            match key.code {
                KeyCode::Esc => self.modal = None,
                KeyCode::Up => {
                    palette.move_selection(-1);
                    self.modal = Some(Modal::CommandPalette(palette));
                }
                KeyCode::Down | KeyCode::Tab => {
                    palette.move_selection(1);
                    self.modal = Some(Modal::CommandPalette(palette));
                }
                KeyCode::Backspace => {
                    palette.backspace();
                    self.modal = Some(Modal::CommandPalette(palette));
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    palette.clear();
                    self.modal = Some(Modal::CommandPalette(palette));
                }
                KeyCode::Enter => {
                    if let Some(command) = palette.selected_command() {
                        self.execute_command(command);
                    }
                }
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    palette.push_char(c);
                    self.modal = Some(Modal::CommandPalette(palette));
                }
                _ => {}
            }
            return;
        }

        if key.code == KeyCode::Char('q') {
            self.should_quit = true;
            return;
        }
        match modal {
            Modal::Help(mut help) => {
                let width = self.size.width;
                let height = self.size.height;
                match key.code {
                    KeyCode::Esc | KeyCode::Char('?') => self.modal = None,
                    KeyCode::Char('j') | KeyCode::Down => {
                        help.scroll_by(1, width, height);
                        self.modal = Some(Modal::Help(help));
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        help.scroll_by(-1, width, height);
                        self.modal = Some(Modal::Help(help));
                    }
                    KeyCode::PageDown => {
                        help.page(true, width, height);
                        self.modal = Some(Modal::Help(help));
                    }
                    KeyCode::PageUp => {
                        help.page(false, width, height);
                        self.modal = Some(Modal::Help(help));
                    }
                    KeyCode::Home => {
                        help.home();
                        self.modal = Some(Modal::Help(help));
                    }
                    KeyCode::End => {
                        help.end(width, height);
                        self.modal = Some(Modal::Help(help));
                    }
                    _ => {}
                }
            }
            Modal::Sort(mut sort_modal) => match key.code {
                KeyCode::Esc => self.modal = None,
                KeyCode::Char('j') | KeyCode::Down => {
                    sort_modal.move_selection(1);
                    self.modal = Some(Modal::Sort(sort_modal));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    sort_modal.move_selection(-1);
                    self.modal = Some(Modal::Sort(sort_modal));
                }
                KeyCode::Char('l') | KeyCode::Right => {
                    if sort_modal.is_range() {
                        sort_modal.cycle_range(true);
                        self.modal = Some(Modal::Sort(sort_modal));
                    }
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    if sort_modal.is_range() {
                        sort_modal.cycle_range(false);
                        self.modal = Some(Modal::Sort(sort_modal));
                    }
                }
                KeyCode::Enter => {
                    let choice = sort_modal.current();
                    match (sort_modal.context, choice) {
                        (SortContext::Feed, Some(SortChoice::Feed(sort))) => {
                            self.apply_feed_sort(sort)
                        }
                        (SortContext::Feed, Some(SortChoice::Range)) => {
                            self.modal =
                                Some(Modal::Range(RangeModal::current(sort_modal.top_range)));
                        }
                        (SortContext::Comments, Some(SortChoice::Comment(sort))) => {
                            self.apply_comment_sort(sort)
                        }
                        _ => self.modal = None,
                    }
                }
                _ => {}
            },
            Modal::Range(mut range_modal) => match key.code {
                KeyCode::Esc => self.modal = None,
                KeyCode::Char('j') | KeyCode::Down => {
                    range_modal.move_selection(1);
                    self.modal = Some(Modal::Range(range_modal));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    range_modal.move_selection(-1);
                    self.modal = Some(Modal::Range(range_modal));
                }
                KeyCode::Enter => {
                    let range = range_modal.value();
                    self.apply_top_range(range);
                }
                _ => {}
            },
            Modal::Text(_) => {}
            Modal::Settings(settings) => match key.code {
                KeyCode::Esc => self.modal = None,
                KeyCode::Char('j') | KeyCode::Down => {
                    let next = (settings.selected + 1).min(3);
                    self.modal = Some(Modal::Settings(SettingsModal {
                        selected: next,
                        theme: settings.theme,
                    }));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.modal = Some(Modal::Settings(SettingsModal {
                        selected: settings.selected.saturating_sub(1),
                        theme: settings.theme,
                    }));
                }
                KeyCode::Enter => self.activate_settings_item(settings.selected),
                _ => {}
            },
            Modal::ThemePicker(picker) => match key.code {
                KeyCode::Esc => {
                    self.apply_theme_preview(picker.original);
                    self.modal = Some(Modal::Settings(SettingsModal {
                        selected: 1,
                        theme: picker.original,
                    }));
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    let next = (picker.selected + 1).min(crate::config::ThemeMode::ALL.len() - 1);
                    let theme = crate::config::ThemeMode::ALL[next];
                    self.apply_theme_preview(theme);
                    self.modal = Some(Modal::ThemePicker(ThemePickerModal {
                        selected: next,
                        original: picker.original,
                    }));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    let next = picker.selected.saturating_sub(1);
                    let theme = crate::config::ThemeMode::ALL[next];
                    self.apply_theme_preview(theme);
                    self.modal = Some(Modal::ThemePicker(ThemePickerModal {
                        selected: next,
                        original: picker.original,
                    }));
                }
                KeyCode::Enter => {
                    let theme = crate::config::ThemeMode::ALL[picker.selected];
                    self.commit_theme(theme);
                    self.modal = Some(Modal::Settings(SettingsModal { selected: 1, theme }));
                }
                _ => {}
            },
            Modal::Bookmarks(mut bookmarks) => match key.code {
                KeyCode::Esc => self.modal = None,
                KeyCode::Char('j') | KeyCode::Down => {
                    let total = self.library.bookmarks.len();
                    bookmarks.selected = (bookmarks.selected + 1).min(total.saturating_sub(1));
                    self.modal = Some(Modal::Bookmarks(bookmarks));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    bookmarks.selected = bookmarks.selected.saturating_sub(1);
                    self.modal = Some(Modal::Bookmarks(bookmarks));
                }
                KeyCode::Char('d') | KeyCode::Delete => {
                    self.remove_bookmark_item(bookmarks.selected);
                }
                KeyCode::Enter => self.open_bookmark_item(bookmarks.selected),
                _ => {}
            },
            Modal::Filters(mut filters) => match key.code {
                KeyCode::Esc => {
                    self.modal = Some(Modal::Settings(SettingsModal::new(
                        self.settings.general.theme,
                    )));
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    let max = self.filter_row_count().saturating_sub(1);
                    filters.selected = (filters.selected + 1).min(max);
                    self.modal = Some(Modal::Filters(filters));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    filters.selected = filters.selected.saturating_sub(1);
                    self.modal = Some(Modal::Filters(filters));
                }
                KeyCode::Enter => {
                    if filters.selected == 0 {
                        self.toggle_hide_read();
                    } else {
                        self.notify(
                            NoticeKind::Info,
                            "Press d to remove this filter, or a to add another.",
                        );
                    }
                }
                KeyCode::Char('a') => {
                    self.modal = Some(Modal::Text(TextModal::add_filter()));
                }
                KeyCode::Char('d') | KeyCode::Delete => {
                    self.remove_filter_row(filters.selected);
                }
                KeyCode::Char('C') => self.clear_all_filters(),
                _ => {}
            },
            Modal::Source(mut source) => match key.code {
                KeyCode::Esc => {
                    self.modal = Some(Modal::Settings(SettingsModal::new(
                        self.settings.general.theme,
                    )));
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    source.move_selection(1);
                    self.modal = Some(Modal::Source(source));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    source.move_selection(-1);
                    self.modal = Some(Modal::Source(source));
                }
                KeyCode::Enter => self.execute_source_action(source.selected),
                KeyCode::Char('r') => self.execute_source_action(0),
                KeyCode::Char('c') => self.execute_source_action(1),
                _ => {}
            },
            Modal::ConfigPicker(picker) => match key.code {
                KeyCode::Esc => {
                    self.modal = Some(Modal::Settings(SettingsModal::new(
                        self.settings.general.theme,
                    )));
                }
                KeyCode::Char('j') | KeyCode::Down => {
                    let next = (picker.selected + 1).min(crate::config::presets::PRESETS.len() - 1);
                    self.modal = Some(Modal::ConfigPicker(ConfigPickerModal { selected: next }));
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    self.modal = Some(Modal::ConfigPicker(ConfigPickerModal {
                        selected: picker.selected.saturating_sub(1),
                    }));
                }
                KeyCode::Enter => self.activate_config_picker_item(picker.selected),
                _ => {}
            },
            Modal::Confirm(confirm) => match key.code {
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                    self.modal = None;
                }
                KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let ConfirmAction::RemoveFavorite(name) = confirm.action.clone();
                    self.remove_favorite_now(&name);
                    self.modal = None;
                }
                _ => {}
            },
            Modal::CommandPalette(_) => {}
        }
    }

    fn activate_sort_modal_item(&mut self, index: usize) {
        let Some(Modal::Sort(mut modal)) = self.modal.clone() else {
            return;
        };
        if index >= modal.len() {
            return;
        }
        modal.selected = index;
        match (modal.context, modal.current()) {
            (SortContext::Feed, Some(SortChoice::Feed(sort))) => self.apply_feed_sort(sort),
            (SortContext::Feed, Some(SortChoice::Range)) => {
                self.modal = Some(Modal::Range(RangeModal::current(modal.top_range)));
            }
            (SortContext::Comments, Some(SortChoice::Comment(sort))) => {
                self.apply_comment_sort(sort)
            }
            _ => self.modal = Some(Modal::Sort(modal)),
        }
    }

    fn activate_range_modal_item(&mut self, index: usize) {
        let Some(Modal::Range(mut modal)) = self.modal.clone() else {
            return;
        };
        if index >= TopRange::ALL.len() {
            return;
        }
        modal.selected = index;
        let range = modal.value();
        self.apply_top_range(range);
    }

    // ------------------------------------------------------------------
    // Layout preparation and image requests
    // ------------------------------------------------------------------

    pub fn set_size(&mut self, width: u16, height: u16) {
        let new = Rect::new(0, 0, width, height);
        if new != self.size {
            self.size = new;
            for view in &mut self.nav {
                match view {
                    View::Feed(f) => f.invalidate_heights(),
                    View::Post(p) => {
                        p.comments_dirty = true;
                        p.invalidate_body();
                    }
                    View::Gallery(_) => {}
                }
            }
            if self.focus == Focus::Sidebar && (!self.settings.general.show_sidebar || width < 76) {
                self.focus = Focus::Feed;
            }
            self.dirty = true;
        }
    }

    pub fn is_hidden(&self, post: &Post) -> bool {
        let revealed = self.revealed.contains(&post.id);
        post.is_hidden(
            self.settings.content.show_nsfw || revealed,
            !self.settings.content.hide_spoilers || revealed,
        )
    }

    pub fn prepare(&mut self) {
        if self.size.width == 0 || self.size.height == 0 {
            return;
        }
        let theme = self.theme();
        let root = layout::root(self.size);

        match self.nav.last() {
            Some(View::Feed(_)) => {
                let feed_layout = layout::feed(root.body, &self.settings, true);
                let list = layout::feed_list(feed_layout.feed).list;
                let (cards, _) = layout::cards_and_scrollbar(list);
                let card_width = cards.width.max(8);
                let compact = post_card::compact_for_viewport(list.height);
                let preview_enabled = self.settings.images.enabled
                    && self.settings.images.protocol
                        != crate::config::ImageProtocolSetting::Disabled
                    && self.settings.images.preview_in_feed;

                {
                    let App {
                        nav,
                        settings,
                        revealed,
                        images,
                        provider,
                        ..
                    } = self;
                    if let Some(View::Feed(feed)) = nav.last_mut() {
                        feed.recompute_heights(
                            card_width,
                            compact,
                            settings,
                            preview_enabled,
                            revealed,
                        );
                        feed.ensure_selected_visible(list.height);
                        feed.last_viewport = list.height;
                        let range = feed.visible_range(list.height);
                        for post in &feed.posts[range] {
                            let hidden = post.is_hidden(
                                settings.content.show_nsfw || revealed.contains(&post.id),
                                !settings.content.hide_spoilers || revealed.contains(&post.id),
                            );
                            let Some(preview) = post.preview_image() else {
                                continue;
                            };
                            if hidden || !preview_enabled {
                                continue;
                            }
                            let card = post_card::card_layout(
                                post,
                                card_width,
                                compact,
                                preview_enabled,
                                hidden,
                            );
                            if card.image_height >= 2 && card.inner_width > 0 {
                                let fetch = provider.resolve_media_url(preview);
                                images.request(
                                    preview,
                                    &fetch,
                                    card.inner_width,
                                    card.image_height,
                                );
                            }
                        }
                    }
                }
                self.request_more_if_needed();
            }
            Some(View::Post(p)) => {
                let hidden = self.is_hidden(&p.post);
                let post_id = p.id;
                let preview = p.post.preview_image().map(str::to_string);
                let has_body = !p.post.body.trim().is_empty();
                let panels = layout::post(root.body, &self.settings);
                let rects = crate::ui::screens::post::layout_for(
                    &p.post,
                    panels.main,
                    &self.settings,
                    hidden,
                );
                if let Some(view) = self.post_mut(post_id) {
                    view.last_comments_viewport = rects.comments_list.height;
                    view.last_body_viewport = rects.body.height;
                    view.comments_dirty = view.comments_dirty || view.last_comment_width == 0;
                    view.rebuild_comments(rects.comments_list.width.saturating_sub(2).max(10));
                    view.ensure_comment_visible(rects.comments_list.height);
                    if !hidden && has_body {
                        view.ensure_body(rects.body.width.max(10), &theme);
                    } else if let Some((w, _)) = &view.body_cache
                        && *w != rects.body.width
                    {
                        view.invalidate_body();
                    }
                    view.clamp_body();
                }
                if let (Some(image_rect), false, Some(preview)) = (rects.image, hidden, preview) {
                    let fetch = self.provider.resolve_media_url(&preview);
                    self.images.request(
                        &preview,
                        &fetch,
                        image_rect.width.max(1),
                        image_rect.height.max(1),
                    );
                }
            }
            Some(View::Gallery(g)) => {
                let rects = layout::gallery(root.body);
                let width = rects.image.width.saturating_sub(2).max(1);
                let height = rects.image.height.saturating_sub(2).max(1);
                if let Some(image) = g.current() {
                    let fetch = self.provider.resolve_media_url(&image.url);
                    self.images.request(&image.url, &fetch, width, height);
                }
            }
            None => {}
        }
    }

    // ------------------------------------------------------------------
    // Keyboard
    // ------------------------------------------------------------------

    pub fn handle_key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        self.dirty = true;

        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('p') {
            if matches!(self.modal, Some(Modal::CommandPalette(_))) {
                self.modal = None;
            } else if self.modal.is_none() {
                self.open_command_palette();
            }
            return;
        }
        if self.modal.is_some() {
            self.handle_modal_key(key);
            return;
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return;
        }

        if key.code == KeyCode::Char('S') {
            self.open_settings();
            return;
        }

        match self.nav.last() {
            Some(View::Feed(_)) => self.handle_feed_key(key),
            Some(View::Post(_)) => self.handle_post_key(key),
            Some(View::Gallery(_)) => self.handle_gallery_key(key),
            None => {}
        }
    }

    fn handle_feed_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('q') {
            self.should_quit = true;
            return;
        }
        match key.code {
            KeyCode::Char('?') => self.open_help(),
            KeyCode::Char(':') => self.open_command_palette(),
            KeyCode::Char('j') | KeyCode::Down if self.focus == Focus::Sidebar => {
                self.move_sidebar(1)
            }
            KeyCode::Char('k') | KeyCode::Up if self.focus == Focus::Sidebar => {
                self.move_sidebar(-1)
            }
            KeyCode::Enter if self.focus == Focus::Sidebar => self.open_sidebar_selected(),
            KeyCode::Esc if self.focus == Focus::Sidebar => self.focus = Focus::Feed,
            KeyCode::Char('h') => self.toggle_sidebar(),
            KeyCode::Tab => self.cycle_focus(true),
            KeyCode::BackTab => self.cycle_focus(false),
            KeyCode::Left => self.focus_left(),
            KeyCode::Right => self.focus_right(),
            KeyCode::Char('f') => self.toggle_favorite(),
            KeyCode::Char('b') => self.toggle_bookmark_current(),
            KeyCode::Char('B') => self.open_bookmarks(),
            KeyCode::Char('m') => self.toggle_read_current(),
            KeyCode::Char('y') => self.copy_permalink(),
            KeyCode::Char('i') => self.copy_image_url(),
            KeyCode::Char('Y') => self.open_comments(),
            KeyCode::Char('G') => self.open_subreddit(),
            KeyCode::Char('T') => self.toggle_theme(),
            KeyCode::Char('s') => self.open_sort_modal(),
            KeyCode::Char('t') => self.open_range_modal(),
            KeyCode::Char('g') => self.open_goto_modal(),
            KeyCode::Char('/') => self.open_search_modal(),
            KeyCode::Char('r') => self.refresh_current(),
            KeyCode::Char('o') => self.open_url(),
            KeyCode::Char('v') => self.open_gallery(),
            KeyCode::Char('x') => self.reveal_current(),
            KeyCode::Esc => self.back(),
            KeyCode::Enter => self.open_post(),
            KeyCode::Char('j') | KeyCode::Down => {
                let viewport = self.current_feed().map(|f| f.last_viewport).unwrap_or(1);
                if let Some(feed) = self.current_feed_mut() {
                    feed.move_selection(1, viewport);
                }
                self.request_more_if_needed();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let viewport = self.current_feed().map(|f| f.last_viewport).unwrap_or(1);
                if let Some(feed) = self.current_feed_mut() {
                    feed.move_selection(-1, viewport);
                }
            }
            KeyCode::PageDown => {
                let viewport = self.current_feed().map(|f| f.last_viewport).unwrap_or(1);
                if let Some(feed) = self.current_feed_mut() {
                    feed.page_move(true, viewport);
                }
                self.request_more_if_needed();
            }
            KeyCode::PageUp => {
                let viewport = self.current_feed().map(|f| f.last_viewport).unwrap_or(1);
                if let Some(feed) = self.current_feed_mut() {
                    feed.page_move(false, viewport);
                }
            }
            KeyCode::Home => {
                let viewport = self.current_feed().map(|f| f.last_viewport).unwrap_or(1);
                if let Some(feed) = self.current_feed_mut() {
                    feed.go_home(viewport);
                }
            }
            KeyCode::End => {
                let viewport = self.current_feed().map(|f| f.last_viewport).unwrap_or(1);
                if let Some(feed) = self.current_feed_mut() {
                    feed.go_end(viewport);
                }
                self.request_more_if_needed();
            }
            _ => {}
        }
    }

    fn handle_post_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('q') {
            self.should_quit = true;
            return;
        }
        if self.focus != Focus::PostBody && self.focus != Focus::Comments {
            self.focus = Focus::Comments;
        }
        match key.code {
            KeyCode::Char('?') => self.open_help(),
            KeyCode::Char(':') => self.open_command_palette(),
            KeyCode::Esc => self.back(),
            KeyCode::Tab | KeyCode::BackTab => self.cycle_focus(true),
            KeyCode::Left => self.focus_left(),
            KeyCode::Right => self.focus_right(),
            KeyCode::Char('s') => self.open_sort_modal(),
            KeyCode::Char('v') | KeyCode::Enter if self.focus == Focus::PostBody => {
                self.open_gallery()
            }
            KeyCode::Enter | KeyCode::Char('c') if self.focus == Focus::Comments => {
                self.toggle_comment()
            }
            KeyCode::Char('v') => self.open_gallery(),
            KeyCode::Char('o') => self.open_url(),
            KeyCode::Char('x') => self.reveal_current(),
            KeyCode::Char('r') => self.refresh_current(),
            KeyCode::Char('g') => self.open_goto_modal(),
            KeyCode::Char('/') => self.open_search_modal(),
            KeyCode::Char('f') => self.toggle_favorite(),
            KeyCode::Char('b') => self.toggle_bookmark_current(),
            KeyCode::Char('B') => self.open_bookmarks(),
            KeyCode::Char('m') => self.toggle_read_current(),
            KeyCode::Char('y') => self.copy_permalink(),
            KeyCode::Char('i') => self.copy_image_url(),
            KeyCode::Char('Y') => self.open_permalink(),
            KeyCode::Char('G') => self.open_subreddit(),
            KeyCode::Char('[') => self.comment_prev_root(),
            KeyCode::Char(']') => self.comment_next_root(),
            KeyCode::Char('p') => self.comment_parent(),
            KeyCode::Char('z') => self.comment_collapse_all(),
            KeyCode::Char('O') => self.comment_toggle_op_only(),
            KeyCode::Char('n') => self.comment_next_unread(),
            KeyCode::Char('h') => self.toggle_sidebar(),
            KeyCode::Char('T') => self.toggle_theme(),
            KeyCode::Char('j') | KeyCode::Down => {
                self.post_move(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.post_move(-1);
            }
            KeyCode::PageDown => self.post_page(true),
            KeyCode::PageUp => self.post_page(false),
            KeyCode::Home => {
                if self.focus == Focus::Comments {
                    let vp = self
                        .current_post()
                        .map(|p| p.last_comments_viewport)
                        .unwrap_or(1);
                    if let Some(p) = self.current_post_mut() {
                        p.comment_home(vp);
                    }
                } else if let Some(p) = self.current_post_mut() {
                    p.body_offset = 0;
                }
            }
            KeyCode::End => {
                if self.focus == Focus::Comments {
                    let vp = self
                        .current_post()
                        .map(|p| p.last_comments_viewport)
                        .unwrap_or(1);
                    if let Some(p) = self.current_post_mut() {
                        p.comment_end(vp);
                    }
                } else if let Some(p) = self.current_post_mut() {
                    p.body_offset = p.body_len() as u32;
                    p.clamp_body();
                }
            }
            _ => {}
        }
    }

    fn post_move(&mut self, delta: isize) {
        if self.focus == Focus::Comments {
            let vp = self
                .current_post()
                .map(|p| p.last_comments_viewport)
                .unwrap_or(1);
            if let Some(p) = self.current_post_mut() {
                p.move_comment_selection(delta, vp);
            }
        } else {
            let step = if delta > 0 { 1 } else { -1 };
            if let Some(p) = self.current_post_mut() {
                p.scroll_body(step);
            }
        }
    }

    fn post_page(&mut self, down: bool) {
        if self.focus == Focus::Comments {
            let vp = self
                .current_post()
                .map(|p| p.last_comments_viewport)
                .unwrap_or(1);
            if let Some(p) = self.current_post_mut() {
                p.comment_page(down, vp);
            }
        } else if let Some(p) = self.current_post_mut() {
            p.body_page(down);
        }
    }

    fn toggle_comment(&mut self) {
        let Some(post) = self.current_post() else {
            return;
        };
        let post_id = post.id;
        let width = post.last_comment_width.max(40);
        let outcome = match self.post_mut(post_id) {
            Some(post) => post.toggle_selected(width),
            None => return,
        };
        match outcome {
            ToggleOutcome::Nothing => {
                let is_more = self
                    .current_post()
                    .and_then(|p| p.rows.get(p.selected))
                    .is_some_and(|r| r.is_more);
                if is_more {
                    self.notify(NoticeKind::Info, "No further comment IDs were provided.");
                }
            }
            ToggleOutcome::Toggled => {}
            ToggleOutcome::LoadMore { id, children } => {
                self.request_more_comments(post_id, id, children);
            }
        }
    }

    fn comment_viewport(&self) -> u16 {
        self.current_post()
            .map(|post| post.last_comments_viewport.max(1))
            .unwrap_or(1)
    }

    fn focus_comments(&mut self) {
        if self.focus == Focus::PostBody {
            self.focus = Focus::Comments;
        }
    }

    fn comment_next_root(&mut self) {
        self.focus_comments();
        let viewport = self.comment_viewport();
        let moved = self
            .current_post_mut()
            .is_some_and(|post| post.next_root(viewport));
        if !moved {
            self.notify(NoticeKind::Info, "No next top-level comment.");
        }
    }

    fn comment_prev_root(&mut self) {
        self.focus_comments();
        let viewport = self.comment_viewport();
        let moved = self
            .current_post_mut()
            .is_some_and(|post| post.prev_root(viewport));
        if !moved {
            self.notify(NoticeKind::Info, "No previous top-level comment.");
        }
    }

    fn comment_parent(&mut self) {
        self.focus_comments();
        let viewport = self.comment_viewport();
        let moved = self
            .current_post_mut()
            .is_some_and(|post| post.jump_to_parent(viewport));
        if !moved {
            self.notify(NoticeKind::Info, "This comment has no parent.");
        }
    }

    fn comment_collapse_all(&mut self) {
        self.focus_comments();
        let Some(post) = self.current_post() else {
            return;
        };
        let post_id = post.id;
        let width = post.last_comment_width.max(40);
        let collapsed = self
            .post_mut(post_id)
            .is_some_and(|post| post.toggle_collapse_all(width));
        if collapsed {
            self.notify(NoticeKind::Info, "Collapsed all comment threads.");
        } else {
            self.notify(NoticeKind::Info, "Expanded all comment threads.");
        }
    }

    fn comment_toggle_op_only(&mut self) {
        self.focus_comments();
        let Some(post) = self.current_post() else {
            return;
        };
        let post_id = post.id;
        let width = post.last_comment_width.max(40);
        let author = post.post.author.clone();
        let op_only = self
            .post_mut(post_id)
            .is_some_and(|post| post.toggle_op_only(width));
        if op_only {
            self.notify(
                NoticeKind::Info,
                format!("Showing only comments by u/{author}."),
            );
        } else {
            self.notify(NoticeKind::Info, "Showing all comments.");
        }
    }

    fn comment_next_unread(&mut self) {
        self.focus_comments();
        let Some(post) = self.current_post() else {
            return;
        };
        let viewport = post.last_comments_viewport.max(1);
        let current_id = post.selected_comment_id().map(str::to_string);
        let next = {
            let App { library, nav, .. } = self;
            if let Some(View::Post(post)) = nav.last_mut() {
                post.next_unread_comment(&library.read_comments, viewport)
            } else {
                None
            }
        };
        let mut changed = false;
        if let Some(id) = current_id {
            changed |= self.library.mark_comment_read(&id);
        }
        match next {
            Some(id) => {
                changed |= self.library.mark_comment_read(&id);
                if changed {
                    self.save_library();
                }
                self.notify(NoticeKind::Info, "Jumped to the next unread comment.");
            }
            None => {
                if changed {
                    self.save_library();
                }
                self.notify(NoticeKind::Info, "No unread comments below this one.");
            }
        }
    }

    /// Absolute permalink for the selected comment, or the post when the body
    /// or the feed is focused.
    fn current_permalink(&self) -> Option<(String, &'static str)> {
        if let Some(View::Post(post)) = self.nav.last() {
            if self.focus == Focus::Comments
                && let Some(comment_id) = post.selected_comment_id()
                && let Some(url) = post.post.comment_url(comment_id)
            {
                return Some((url, "Comment permalink"));
            }
            return post.post.reddit_url().map(|url| (url, "Post permalink"));
        }
        let post = match self.nav.last() {
            Some(View::Feed(feed)) => feed.selected_post(),
            Some(View::Gallery(gallery)) => Some(&gallery.post),
            _ => None,
        }?;
        post.reddit_url().map(|url| (url, "Post permalink"))
    }

    fn copy_permalink(&mut self) {
        let Some((url, label)) = self.current_permalink() else {
            self.notify(NoticeKind::Info, "This item has no permalink.");
            return;
        };
        match crate::utils::clipboard::copy(&url) {
            Ok(()) => self.notify(NoticeKind::Success, format!("{label} copied to clipboard.")),
            Err(error) => self.notify(NoticeKind::Error, error),
        }
    }

    fn open_permalink(&mut self) {
        let Some((url, label)) = self.current_permalink() else {
            self.notify(NoticeKind::Info, "This item has no permalink.");
            return;
        };
        match browser::open_url(&url) {
            Ok(()) => self.notify(
                NoticeKind::Success,
                format!("Opened {label} in your browser."),
            ),
            Err(error) => self.notify(NoticeKind::Error, error),
        }
    }

    /// Comments-thread URL for the current post. This is the post permalink on
    /// Reddit (the page that contains the comments).
    pub fn comments_url(&self) -> Option<String> {
        self.action_post().and_then(Post::reddit_url)
    }

    /// Browser URL for the active/selected subreddit, validated before use.
    pub fn subreddit_url(&self) -> Option<String> {
        let name = self.current_subreddit_name()?;
        let name = normalize_subreddit(&name)?;
        Some(format!("https://www.reddit.com/r/{name}"))
    }

    /// External URL of the current gallery image, or the selected/open post's
    /// preview image when not in the gallery viewer.
    pub fn current_image_url(&self) -> Option<String> {
        match self.nav.last() {
            Some(View::Gallery(gallery)) => gallery.current().map(|item| item.url.clone()),
            Some(View::Post(post)) => post.post.preview_image().map(str::to_string),
            Some(View::Feed(feed)) => feed
                .selected_post()
                .and_then(Post::preview_image)
                .map(str::to_string),
            None => None,
        }
    }

    fn copy_image_url(&mut self) {
        let Some(url) = self.current_image_url() else {
            self.notify(NoticeKind::Info, "This item has no image URL.");
            return;
        };
        if url.starts_with("demo://") {
            self.notify(
                NoticeKind::Info,
                "Demo images are local fixtures; there is no external image URL.",
            );
            return;
        }
        match crate::utils::clipboard::copy(&url) {
            Ok(()) => self.notify(NoticeKind::Success, "Image URL copied to clipboard."),
            Err(error) => self.notify(NoticeKind::Error, error),
        }
    }

    fn open_comments(&mut self) {
        let Some(url) = self.comments_url() else {
            self.notify(NoticeKind::Info, "This item has no comments page.");
            return;
        };
        match browser::open_url(&url) {
            Ok(()) => self.notify(NoticeKind::Success, "Opened comments in your browser."),
            Err(error) => self.notify(NoticeKind::Error, error),
        }
    }

    fn open_subreddit(&mut self) {
        let Some(url) = self.subreddit_url() else {
            self.notify(
                NoticeKind::Info,
                "Open a subreddit or select a sidebar entry first.",
            );
            return;
        };
        let label = self
            .current_subreddit_name()
            .map(|name| format!("r/{name}"))
            .unwrap_or_else(|| "subreddit".into());
        match browser::open_url(&url) {
            Ok(()) => self.notify(
                NoticeKind::Success,
                format!("Opened {label} in your browser."),
            ),
            Err(error) => self.notify(NoticeKind::Error, error),
        }
    }

    fn gallery_step(&mut self, forward: bool) {
        if let Some(gallery) = self.current_gallery_mut() {
            if forward {
                gallery.next();
            } else {
                gallery.prev();
            }
            // A full viewport clear also removes sixel/iTerm2 residues in
            // terminals where overwriting cells is not enough.
            self.force_clear = true;
            self.dirty = true;
        }
    }

    fn handle_gallery_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('q') {
            self.should_quit = true;
            return;
        }
        match key.code {
            KeyCode::Esc => self.back(),
            KeyCode::Char('?') => self.open_help(),
            KeyCode::Char(':') => self.open_command_palette(),
            KeyCode::Left | KeyCode::Char('h') => self.gallery_step(false),
            KeyCode::Right | KeyCode::Char('l') => self.gallery_step(true),
            KeyCode::Char('o') => self.open_url(),
            KeyCode::Char('y') | KeyCode::Char('i') => self.copy_image_url(),
            KeyCode::Char('Y') => self.open_comments(),
            KeyCode::Char('G') => self.open_subreddit(),
            KeyCode::Char('b') => self.toggle_bookmark_current(),
            KeyCode::Char('B') => self.open_bookmarks(),
            KeyCode::Char('r') => self.refresh_current(),
            KeyCode::Char('T') => self.toggle_theme(),
            _ => {}
        }
    }

    /// Move focus to the pane on the left: feed → sidebar, comments → post.
    fn focus_left(&mut self) {
        match self.nav.last() {
            Some(View::Feed(_)) => {
                if self.focus == Focus::Feed {
                    if self.size.width < 76 {
                        self.notify(
                            NoticeKind::Info,
                            "The sidebar needs at least 76 columns; widen the terminal to use it.",
                        );
                        return;
                    }
                    if !self.settings.general.show_sidebar {
                        self.settings.general.show_sidebar = true;
                        self.persist();
                    }
                    self.focus = Focus::Sidebar;
                    self.clamp_sidebar();
                    self.dirty = true;
                }
            }
            Some(View::Post(_)) if self.focus == Focus::Comments => {
                self.focus = Focus::PostBody;
                self.dirty = true;
            }
            _ => {}
        }
    }

    /// Move focus to the pane on the right: sidebar → feed, post → comments.
    fn focus_right(&mut self) {
        match self.nav.last() {
            Some(View::Feed(_)) => {
                if self.focus == Focus::Sidebar {
                    self.focus = Focus::Feed;
                    self.dirty = true;
                }
            }
            Some(View::Post(_)) if self.focus == Focus::PostBody => {
                self.focus = Focus::Comments;
                self.dirty = true;
            }
            _ => {}
        }
    }

    fn cycle_focus(&mut self, _forward: bool) {
        match self.nav.last() {
            Some(View::Feed(_)) => {
                let sidebar_visible = self.settings.general.show_sidebar && self.size.width >= 76;
                self.focus = if self.focus == Focus::Feed && sidebar_visible {
                    Focus::Sidebar
                } else {
                    Focus::Feed
                };
            }
            Some(View::Post(_)) => {
                self.focus = if self.focus == Focus::Comments {
                    Focus::PostBody
                } else {
                    Focus::Comments
                };
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Sidebar
    // ------------------------------------------------------------------

    pub fn sidebar_entries(&self) -> Vec<SidebarEntry> {
        sidebar::entries(&self.settings, self.provider.capabilities())
    }

    pub fn sidebar_selected_entry(&self) -> Option<SidebarEntry> {
        self.sidebar_entries().get(self.sidebar.selected).cloned()
    }

    pub fn move_sidebar(&mut self, delta: isize) {
        let total = self.sidebar_entries().len();
        if total == 0 {
            self.sidebar.selected = 0;
            return;
        }
        let max = total - 1;
        self.sidebar.selected = if delta < 0 {
            self.sidebar.selected.saturating_sub(delta.unsigned_abs())
        } else {
            (self.sidebar.selected + delta as usize).min(max)
        };
    }

    pub fn open_sidebar_selected(&mut self) {
        if let Some(entry) = self.sidebar_selected_entry() {
            self.open_feed(entry.spec, None, true);
        }
    }

    pub fn clamp_sidebar(&mut self) {
        let total = self.sidebar_entries().len();
        self.sidebar.selected = sidebar::clamp_selection(self.sidebar.selected, total);
    }

    // ------------------------------------------------------------------
    // Mouse
    // ------------------------------------------------------------------

    pub fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::Moved => {
                self.dirty = false;
            }
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                let direction: i32 = if matches!(mouse.kind, MouseEventKind::ScrollDown) {
                    1
                } else {
                    -1
                };
                if matches!(self.modal, Some(Modal::Help(_))) {
                    let width = self.size.width;
                    let height = self.size.height;
                    if let Some(Modal::Help(help)) = &mut self.modal {
                        help.scroll_by(direction as isize * 3, width, height);
                    }
                    self.dirty = true;
                    return;
                }
                if self.modal.is_some() {
                    self.dirty = false;
                    return;
                }
                self.scroll_pane(mouse.column, mouse.row, direction);
                self.dirty = true;
            }
            MouseEventKind::Down(MouseButton::Left) => {
                self.dirty = true;
                let target = self.hit_map.hit_at(mouse.column, mouse.row).cloned();
                if self.modal.is_some() {
                    self.handle_modal_click(target);
                    return;
                }
                let Some(target) = target else {
                    return;
                };
                self.handle_hit(target);
            }
            _ => {}
        }
    }

    fn scroll_pane(&mut self, x: u16, y: u16, direction: i32) {
        let pane = self.hit_map.pane_at(x, y).or(match self.focus {
            Focus::Sidebar => Some(Pane::Sidebar),
            Focus::Feed => Some(Pane::FeedList),
            Focus::PostBody => Some(Pane::PostBody),
            Focus::Comments => Some(Pane::Comments),
        });
        match pane {
            Some(Pane::Sidebar) => self.move_sidebar(-direction as isize),
            Some(Pane::FeedList) => {
                let vp = self.current_feed().map(|f| f.last_viewport).unwrap_or(1);
                if let Some(feed) = self.current_feed_mut() {
                    feed.scroll_lines(direction * 3, vp);
                }
                self.request_more_if_needed();
            }
            Some(Pane::Comments) => {
                let vp = self
                    .current_post()
                    .map(|p| p.last_comments_viewport)
                    .unwrap_or(1);
                if let Some(post) = self.current_post_mut() {
                    post.scroll_comments(direction * 3, vp);
                }
            }
            Some(Pane::PostBody) => {
                if let Some(post) = self.current_post_mut() {
                    let step = (post.last_body_viewport.max(2) as i32 / 2) * direction;
                    post.scroll_body(step);
                }
            }
            Some(Pane::Gallery) => self.gallery_step(direction > 0),
            None => {}
        }
    }

    fn handle_modal_click(&mut self, target: Option<HitTarget>) {
        match target {
            None | Some(HitTarget::ModalClose) => {
                if let Some(Modal::ThemePicker(picker)) = &self.modal {
                    self.apply_theme_preview(picker.original);
                }
                self.modal = None;
            }
            Some(HitTarget::ModalBody) => {}
            Some(HitTarget::ModalItem(index)) => {
                if matches!(self.modal, Some(Modal::Range(_))) {
                    self.activate_range_modal_item(index);
                } else if matches!(self.modal, Some(Modal::Sort(_))) {
                    self.activate_sort_modal_item(index);
                } else if matches!(self.modal, Some(Modal::Settings(_))) {
                    self.activate_settings_item(index);
                } else if matches!(self.modal, Some(Modal::ConfigPicker(_))) {
                    self.activate_config_picker_item(index);
                } else if matches!(self.modal, Some(Modal::ThemePicker(_)))
                    && let Some(theme) = crate::config::ThemeMode::ALL.get(index).copied()
                {
                    self.commit_theme(theme);
                    self.modal = Some(Modal::Settings(SettingsModal { selected: 1, theme }));
                } else if matches!(self.modal, Some(Modal::Bookmarks(_))) {
                    self.open_bookmark_item(index);
                } else if matches!(self.modal, Some(Modal::Source(_))) {
                    self.execute_source_action(index);
                } else if matches!(self.modal, Some(Modal::Filters(_))) {
                    if index == 0 {
                        self.toggle_hide_read();
                    } else if let Some(Modal::Filters(modal)) = &mut self.modal {
                        modal.selected = index;
                    }
                } else if let Some(Modal::CommandPalette(palette)) = &self.modal
                    && let Some(command) = palette.filtered().get(index).copied()
                {
                    self.execute_command(command);
                }
            }
            Some(HitTarget::ModalRangeItem(_)) => {
                let top = match &self.modal {
                    Some(Modal::Sort(m)) => m.top_range,
                    _ => TopRange::Week,
                };
                self.modal = Some(Modal::Range(RangeModal::current(top)));
            }
            Some(HitTarget::ModalScope) => {
                if let Some(Modal::Text(m)) = &mut self.modal
                    && m.kind == TextKind::Search
                {
                    m.toggle_scope();
                }
            }
            Some(HitTarget::ModalSubmit) => self.submit_text_modal(),
            _ => {}
        }
    }

    fn handle_hit(&mut self, target: HitTarget) {
        let now = Instant::now();
        let double = self.last_click.as_ref().is_some_and(|(at, previous)| {
            *previous == target && now.duration_since(*at) < Duration::from_millis(450)
        });
        if double {
            self.last_click = None;
        } else {
            self.last_click = Some((now, target.clone()));
        }

        match target {
            HitTarget::Pane(pane) => {
                self.focus = match pane {
                    Pane::Sidebar => Focus::Sidebar,
                    Pane::FeedList => Focus::Feed,
                    Pane::PostBody => Focus::PostBody,
                    Pane::Comments => Focus::Comments,
                    Pane::Gallery => self.focus,
                };
            }
            HitTarget::Sidebar(spec) => {
                self.open_feed(spec, None, true);
            }
            HitTarget::FeedPost(index) => {
                let vp = self.current_feed().map(|f| f.last_viewport).unwrap_or(1);
                if let Some(feed) = self.current_feed_mut() {
                    feed.selected = index.min(feed.posts.len().saturating_sub(1));
                    feed.ensure_selected_visible(vp);
                }
                self.focus = Focus::Feed;
                if double {
                    self.open_post();
                }
            }
            HitTarget::FeedSort(sort) => {
                self.focus = Focus::Feed;
                self.apply_feed_sort(sort);
            }
            HitTarget::CommentRow(index) => {
                let vp = self
                    .current_post()
                    .map(|p| p.last_comments_viewport)
                    .unwrap_or(1);
                if let Some(post) = self.current_post_mut() {
                    post.click_comment(index, vp);
                }
                self.focus = Focus::Comments;
                if double {
                    self.toggle_comment();
                }
            }
            HitTarget::GalleryPrev => self.gallery_step(false),
            HitTarget::GalleryNext => self.gallery_step(true),
            HitTarget::GalleryOpen => self.open_url(),
            HitTarget::CommentSort => self.open_sort_modal(),
            HitTarget::OpenImage => self.open_gallery(),
            HitTarget::Retry => self.refresh_current(),
            _ => {}
        }
    }

    // ------------------------------------------------------------------
    // Async events
    // ------------------------------------------------------------------

    pub fn handle_event(&mut self, event: AppEvent) {
        self.dirty = true;
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::Mouse(mouse) => self.handle_mouse(mouse),
            AppEvent::Resize(width, height) => self.set_size(width, height),
            AppEvent::Paste(text) => {
                if let Some(Modal::Text(modal)) = &mut self.modal {
                    modal.paste(&text);
                }
            }
            AppEvent::Tick => {
                self.spinner = (self.spinner + 1) % crate::ui::SPINNER.len();
                let now = Instant::now();
                let before = self.notifications.len();
                self.notifications.retain(|n| !n.expired(now));
                let expired = before != self.notifications.len();
                self.dirty = expired || self.animating();
            }
            AppEvent::FeedLoaded {
                view,
                request,
                append,
                result,
            } => self.handle_feed_loaded(view, request, append, result),
            AppEvent::PostLoaded {
                view,
                request,
                result,
            } => self.handle_post_loaded(view, request, result),
            AppEvent::CommentsLoaded {
                view,
                request,
                result,
            } => self.handle_comments_loaded(view, request, result),
            AppEvent::MoreCommentsLoaded {
                view,
                request,
                more_id,
                result,
            } => self.handle_more_loaded(view, request, more_id, result),
            AppEvent::SubredditLoaded {
                view,
                request,
                name,
                result,
            } => self.handle_subreddit_loaded(view, request, name, result),
            AppEvent::ImageReady { key, result } => {
                if let Err(error) = &result {
                    tracing::debug!(
                        url = %crate::utils::text::sanitize(&key.url),
                        error = %crate::utils::text::sanitize(error),
                        "image unavailable"
                    );
                }
                self.images.on_result(key, result);
            }
        }
    }

    fn handle_feed_loaded(
        &mut self,
        view_id: ViewId,
        request: u64,
        append: bool,
        result: Result<FeedPage, ProviderError>,
    ) {
        tracing::debug!(?view_id, request, append, "handling FeedLoaded");
        let provider_label = self.provider.display_name();
        let status = result
            .as_ref()
            .map_or_else(crate::providers::ProviderStatus::from_error, |_| {
                crate::providers::ProviderStatus::Available
            });
        let error_message = result.as_ref().err().map(ToString::to_string);
        let mut filtered_count = 0usize;
        let result = result.map(|mut page| {
            let before = page.posts.len();
            page.posts.retain(|post| {
                !self
                    .library
                    .filters
                    .hides(post, self.library.is_post_read(&post.id))
            });
            filtered_count = before.saturating_sub(page.posts.len());
            page
        });
        let mut error_toast = None;
        let mut need_subreddit = None;
        {
            let Some(view) = self.feed_mut(view_id) else {
                tracing::debug!("FeedLoaded for unknown view");
                return;
            };
            if append {
                if view.more_request != Some(request) {
                    tracing::debug!("stale append request");
                    self.request_starts.remove(&request);
                    return;
                }
                view.more_request = None;
                match result {
                    Ok(page) => view.apply_page(page, true, false),
                    Err(error) => {
                        view.loading_more = false;
                        view.has_more = false;
                        error_toast = Some(error);
                    }
                }
            } else {
                if view.request != Some(request) {
                    tracing::debug!(current = ?view.request, "stale initial request");
                    self.request_starts.remove(&request);
                    return;
                }
                view.request = None;
                let reset = !view.refreshing;
                match result {
                    Ok(page) => {
                        view.apply_page(page, false, reset);
                        need_subreddit = view.spec.subreddit_name().map(str::to_string);
                    }
                    Err(error) => {
                        view.loading_initial = false;
                        view.refreshing = false;
                        if view.posts.is_empty() {
                            let message =
                                describe_feed_error(&provider_label, &view.spec, view.sort, &error);
                            view.set_error(message);
                        } else {
                            error_toast = Some(error);
                        }
                    }
                }
            }
        }
        if let Some(error) = error_toast {
            self.notify_error(&error);
        }
        if !append && filtered_count > 0 {
            self.notify(
                NoticeKind::Info,
                format!("{filtered_count} post(s) hidden by local filters."),
            );
        }
        self.finish_request(request, status, error_message);
        if let Some(name) = need_subreddit {
            if let Some(view) = self.feed_mut(view_id)
                && view
                    .subreddit
                    .as_ref()
                    .is_none_or(|s| !s.name.eq_ignore_ascii_case(&name))
            {
                view.subreddit = None;
            }
            self.request_subreddit_if_needed(view_id);
        }
        self.request_more_if_needed();
    }

    fn handle_post_loaded(
        &mut self,
        view_id: ViewId,
        request: u64,
        result: Result<Post, ProviderError>,
    ) {
        let Some(view) = self.post_mut(view_id) else {
            return;
        };
        if view.post_request != Some(request) {
            self.request_starts.remove(&request);
            return;
        }
        view.post_request = None;
        view.post_loading = false;
        let status = result
            .as_ref()
            .map_or_else(crate::providers::ProviderStatus::from_error, |_| {
                crate::providers::ProviderStatus::Available
            });
        let error_message = result.as_ref().err().map(ToString::to_string);
        match result {
            Ok(post) => {
                view.post = post;
                view.invalidate_body();
            }
            Err(error) => {
                view.post_error = Some(error.to_string());
                tracing::warn!(
                    error = %crate::utils::text::sanitize(&error.to_string()),
                    "failed to refresh post"
                );
            }
        }
        self.finish_request(request, status, error_message);
    }

    fn handle_comments_loaded(
        &mut self,
        view_id: ViewId,
        request: u64,
        result: Result<Vec<Comment>, ProviderError>,
    ) {
        let Some(view) = self.post_mut(view_id) else {
            return;
        };
        if view.comments_request != Some(request) {
            self.request_starts.remove(&request);
            return;
        }
        view.comments_request = None;
        let width = view.last_comment_width.max(40);
        let status = result
            .as_ref()
            .map_or_else(crate::providers::ProviderStatus::from_error, |_| {
                crate::providers::ProviderStatus::Available
            });
        let error_message = result.as_ref().err().map(ToString::to_string);
        let error = match result {
            Ok(comments) => {
                view.replace_comments(comments, width);
                None
            }
            Err(error) => {
                view.set_comments_error(error.to_string());
                Some(error)
            }
        };
        if let Some(error) = error {
            self.notify_error(&error);
        }
        self.finish_request(request, status, error_message);
    }

    fn handle_more_loaded(
        &mut self,
        view_id: ViewId,
        request: u64,
        more_id: String,
        result: Result<Vec<Comment>, ProviderError>,
    ) {
        let Some(view) = self.post_mut(view_id) else {
            return;
        };
        if view.pending_more.get(&more_id) != Some(&request) {
            self.request_starts.remove(&request);
            return;
        }
        let width = view.last_comment_width.max(40);
        let status = result
            .as_ref()
            .map_or_else(crate::providers::ProviderStatus::from_error, |_| {
                crate::providers::ProviderStatus::Available
            });
        let error_message = result.as_ref().err().map(ToString::to_string);
        let notice = match result {
            Ok(loaded) => {
                let empty = loaded.is_empty();
                view.apply_more(&more_id, loaded, width);
                empty.then(|| {
                    (
                        NoticeKind::Info,
                        "No more replies were returned.".to_string(),
                    )
                })
            }
            Err(error) => {
                view.pending_more.remove(&more_id);
                view.mark_more_loading(&more_id, false, width);
                Some((NoticeKind::Error, error.to_string()))
            }
        };
        if let Some((kind, text)) = notice {
            self.notify(kind, text);
        }
        self.finish_request(request, status, error_message);
    }

    fn handle_subreddit_loaded(
        &mut self,
        view_id: ViewId,
        request: u64,
        name: String,
        result: Result<Subreddit, ProviderError>,
    ) {
        let post_exists = self
            .nav
            .iter()
            .any(|view| matches!(view, View::Post(post) if post.id == view_id));
        if post_exists {
            let target = self.nav.iter().any(|view| {
                matches!(view, View::Post(post) if post.id == view_id && post.subreddit_request == Some(request))
            });
            if target {
                self.handle_post_subreddit_loaded(view_id, request, name, result);
            } else {
                self.request_starts.remove(&request);
            }
            return;
        }
        let Some(view) = self.feed_mut(view_id) else {
            return;
        };
        if view.subreddit_request != Some(request) {
            self.request_starts.remove(&request);
            return;
        }
        view.subreddit_request = None;
        let status = result
            .as_ref()
            .map_or_else(crate::providers::ProviderStatus::from_error, |_| {
                crate::providers::ProviderStatus::Available
            });
        let error_message = result.as_ref().err().map(ToString::to_string);
        match result {
            Ok(subreddit) => {
                if subreddit.name.eq_ignore_ascii_case(&name) {
                    view.subreddit = Some(subreddit);
                    view.subreddit_error = None;
                }
            }
            Err(error) => {
                view.subreddit_error = Some(error.to_string());
            }
        }
        self.finish_request(request, status, error_message);
    }

    fn handle_post_subreddit_loaded(
        &mut self,
        view_id: ViewId,
        request: u64,
        name: String,
        result: Result<Subreddit, ProviderError>,
    ) {
        let Some(view) = self.post_mut(view_id) else {
            return;
        };
        if view.subreddit_request != Some(request) {
            self.request_starts.remove(&request);
            return;
        }
        view.subreddit_request = None;
        let status = result
            .as_ref()
            .map_or_else(crate::providers::ProviderStatus::from_error, |_| {
                crate::providers::ProviderStatus::Available
            });
        let error_message = result.as_ref().err().map(ToString::to_string);
        match result {
            Ok(subreddit) => {
                if subreddit.name.eq_ignore_ascii_case(&name) {
                    view.subreddit = Some(subreddit);
                    view.subreddit_error = None;
                }
            }
            Err(error) => {
                view.subreddit_error = Some(error.to_string());
            }
        }
        self.finish_request(request, status, error_message);
    }
}

/// HTML sources can fail per-sort (for example a Redlib instance whose Reddit
/// backend blocks `new` but still serves `hot`). Keep the provider error type
/// visible but explain what the user can do, and never imply a silent fallback.
fn describe_feed_error(
    provider_label: &str,
    spec: &FeedSpec,
    sort: FeedSort,
    error: &ProviderError,
) -> String {
    if !(provider_label.starts_with("REDLIB") || provider_label.starts_with("OLD REDDIT")) {
        return error.to_string();
    }
    let reddit_source = if provider_label.starts_with("REDLIB") {
        "The Redlib instance's Reddit backend"
    } else {
        "Reddit"
    };
    let reason = match error {
        ProviderError::Forbidden(_) => {
            format!("{reddit_source} refused this listing (access denied or CAPTCHA).")
        }
        ProviderError::NotFound(_) => {
            "The source reported that this listing does not exist or is blocked.".to_string()
        }
        ProviderError::RateLimited { .. } => {
            "The source is currently rate limiting requests.".to_string()
        }
        ProviderError::HtmlStructureChanged(_) => {
            "The source returned HTML that does not match the expected layout.".to_string()
        }
        ProviderError::Parse(_) => "The source returned an unexpected page.".to_string(),
        ProviderError::LoginRequired(_) | ProviderError::AuthRequired(_) => {
            "The source requires a login, which HTML mode cannot provide.".to_string()
        }
        _ => return error.to_string(),
    };
    format!(
        "{} is not available for {} from {provider_label} right now. {reason} Other sorts such as Hot may still work. Try another sort or a different permitted source; ReddTTY does not bypass access controls or switch instances automatically.",
        sort.label(),
        spec.title()
    )
}

fn default_spec(settings: &Settings) -> FeedSpec {
    let raw = settings.general.default_feed.trim();
    if raw.eq_ignore_ascii_case("popular") {
        FeedSpec::Popular
    } else if raw.eq_ignore_ascii_case("home")
        || raw.eq_ignore_ascii_case("frontpage")
        || raw.eq_ignore_ascii_case("best")
        || raw.is_empty()
    {
        FeedSpec::Home
    } else if let Some(name) = normalize_subreddit(raw) {
        FeedSpec::Subreddit(name)
    } else {
        FeedSpec::Home
    }
}

fn modal_is_text(modal: &Modal) -> bool {
    matches!(modal, Modal::Text(_))
}

/// Spawn a provider request when a Tokio runtime is available. Unit tests can
/// exercise state transitions without constructing a runtime; in the real app
/// the runtime always exists.
fn spawn_guarded<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(future);
    }
}

#[cfg(test)]
mod tests;
