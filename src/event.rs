//! Events flowing into the UI. Every asynchronous result carries the view and
//! request id it belongs to, so a late response can never corrupt a new screen.

use crossterm::event::{KeyEvent, MouseEvent};
use ratatui_image::protocol::Protocol;

use crate::error::ProviderError;
use crate::media::ImageKey;
use crate::models::{Comment, FeedPage, Post, Subreddit};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId(pub u64);

pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Paste(String),
    Tick,
    FeedLoaded {
        view: ViewId,
        request: u64,
        append: bool,
        result: Result<FeedPage, ProviderError>,
    },
    PostLoaded {
        view: ViewId,
        request: u64,
        result: Result<Post, ProviderError>,
    },
    CommentsLoaded {
        view: ViewId,
        request: u64,
        result: Result<Vec<Comment>, ProviderError>,
    },
    MoreCommentsLoaded {
        view: ViewId,
        request: u64,
        more_id: String,
        result: Result<Vec<Comment>, ProviderError>,
    },
    SubredditLoaded {
        view: ViewId,
        request: u64,
        name: String,
        result: Result<Subreddit, ProviderError>,
    },
    ImageReady {
        key: ImageKey,
        result: Result<Protocol, String>,
    },
    /// Result of an explicit, bounded source-health probe. It never mutates
    /// the active feed or navigation state.
    ProbeFinished {
        request: u64,
        result: Result<FeedPage, ProviderError>,
    },
    /// Result of the bounded pre-flight test performed before replacing the
    /// active provider configuration.
    SwitchProbeFinished {
        request: u64,
        result: Result<FeedPage, ProviderError>,
    },
}
