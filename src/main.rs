use std::fs::OpenOptions;
use std::io::{IsTerminal, Write, stdout};

use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use crossterm::cursor::Show;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::SetTitle;
use ratatui_image::picker::cap_parser::QueryStdioOptions;
use ratatui_image::picker::{Picker, ProtocolType};
use reddtty::app::App;
use reddtty::cli::{Cli, ThemeArg};
use reddtty::config::{ImageProtocolSetting, ProviderKind, Settings, ThemeMode};
use reddtty::event::AppEvent;
use reddtty::media::ImageManager;
use reddtty::models::{FeedSort, FeedSpec};
use reddtty::providers;
use reddtty::utils::text::normalize_subreddit;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::writer::BoxMakeWriter;

mod setup;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("reddtty: {error:#}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();
    init_logging();

    let (config_path, mut warnings) = if let Some(path) = cli.config.clone() {
        (path, Vec::new())
    } else {
        Settings::migrate_default_config()
    };
    let (mut settings, load_warnings) = Settings::load(&config_path);
    warnings.extend(load_warnings);

    let first_run = cli.config.is_none()
        && !cli.demo
        && cli.provider.is_none()
        && !cli.diagnostics
        && !config_path.exists()
        && stdout().is_terminal();
    if first_run {
        install_panic_hook();
        let mut terminal = ratatui::try_init().context("could not open the setup screen")?;
        let selected = setup::run(&mut terminal);
        let _ = ratatui::try_restore();
        std::thread::sleep(Duration::from_millis(40));
        match selected.context("could not read the setup screen input")? {
            Some(id) => {
                let installed = reddtty::config::presets::install(id, &config_path)
                    .map_err(anyhow::Error::msg)?;
                warnings.push(format!(
                    "Installed config: {}{}.",
                    installed.destination.display(),
                    installed
                        .backup
                        .as_ref()
                        .map(|backup| format!(
                            " (previous config backed up to {})",
                            backup.display()
                        ))
                        .unwrap_or_default()
                ));
                let (reloaded, reload_warnings) = Settings::load(&config_path);
                settings = reloaded;
                warnings.extend(reload_warnings);
            }
            None => {
                settings.provider.kind = ProviderKind::Demo;
                warnings.push("No config installed; using demo data for this session.".into());
            }
        }
    }

    let config_theme = settings.general.theme;
    let config_images_enabled = settings.images.enabled;
    let config_images_protocol = settings.images.protocol;

    if cli.demo
        && cli
            .provider
            .is_some_and(|p| p != reddtty::cli::ProviderArg::Demo)
    {
        bail!("--demo and --provider select different sources; choose one");
    }
    if cli.demo {
        settings.provider.kind = ProviderKind::Demo;
    } else if let Some(provider) = cli.provider {
        settings.provider.kind = provider.into();
    }
    if let Some(instance) = &cli.redlib_instance {
        settings.provider.html.redlib_instance =
            reddtty::config::validate_html_base(instance).map_err(anyhow::Error::msg)?;
    }
    if let Some(theme) = cli.theme {
        settings.general.theme = match theme {
            ThemeArg::Dark => ThemeMode::Dark,
            ThemeArg::Light => ThemeMode::Light,
            ThemeArg::Latte => ThemeMode::Latte,
            ThemeArg::Frappe => ThemeMode::Frappe,
            ThemeArg::Macchiato => ThemeMode::Macchiato,
            ThemeArg::Mocha => ThemeMode::Mocha,
            ThemeArg::HighContrast => ThemeMode::HighContrast,
            ThemeArg::Mono => ThemeMode::Mono,
        };
    }
    if cli.no_color || std::env::var_os("NO_COLOR").is_some() {
        settings.general.theme = ThemeMode::Mono;
        warnings.push("NO_COLOR/no-color: using the monochrome theme for this session.".into());
    }
    if cli.no_images {
        settings.images.enabled = false;
    }
    if let Some(protocol) = cli.images_protocol {
        settings.images.protocol = protocol.into();
        if protocol == reddtty::cli::ProtocolArg::Disabled {
            settings.images.enabled = false;
        }
    }

    let initial_spec = match &cli.subreddit {
        Some(raw) => match normalize_subreddit(raw) {
            Some(name) => Some(FeedSpec::Subreddit(name)),
            None => bail!("invalid --subreddit value `{raw}`"),
        },
        None => None,
    };
    let initial_sort = match &cli.sort {
        Some(raw) => match FeedSort::from_str_loose(raw) {
            Some(sort) => Some(sort),
            None => bail!(
                "invalid --sort value `{raw}` (expected best, hot, new, top, rising or controversial)"
            ),
        },
        None => None,
    };

    if cli.diagnostics {
        print_diagnostics(&settings, &cli, &config_path, &initial_spec, &warnings);
        return Ok(());
    }

    tracing::info!(
        config = %reddtty::utils::text::sanitize(&config_path.display().to_string()),
        demo = cli.demo,
        provider = settings.provider.kind.label(),
        "starting reddtty"
    );
    let setup = providers::build_provider(&settings, cli.demo);
    let mut notices = warnings;
    notices.extend(setup.notices);

    let (tx, rx) = unbounded_channel();
    install_panic_hook();
    let mut terminal = match ratatui::try_init() {
        Ok(terminal) => terminal,
        Err(error) => {
            let _ = ratatui::try_restore();
            return Err(error).context("could not initialise the terminal");
        }
    };

    // Install cleanup before anything else can fail. If enabling mouse capture
    // fails, this guard still restores raw mode and the alternate screen.
    let mut session = SessionGuard { mouse: false };
    let mouse = settings.ui.mouse_enabled;
    if mouse {
        execute!(stdout(), EnableMouseCapture).context("could not enable mouse capture")?;
        session.mouse = true;
    }

    // Give the user something to look at while the terminal image-protocol
    // query runs. Some terminals take a couple of seconds to answer.
    terminal
        .draw(|frame| {
            let block = ratatui::widgets::Paragraph::new(
                "REDDTTY\n\nStarting…  querying terminal capabilities",
            )
            .alignment(ratatui::layout::Alignment::Center)
            .style(
                ratatui::style::Style::default()
                    .fg(ratatui::style::Color::Rgb(255, 92, 20))
                    .add_modifier(ratatui::style::Modifier::BOLD),
            );
            frame.render_widget(block, frame.area());
        })
        .context("could not draw the startup screen")?;

    // The picker must query stdio before the input thread starts reading events.
    let mut picker_notices = Vec::new();
    let picker = make_picker(&settings, &mut picker_notices);
    notices.extend(picker_notices);

    let mut picker = picker;
    picker.set_background_color(Some(image::Rgba(
        reddtty::ui::theme::Theme::background_rgba(settings.general.theme),
    )));
    let images = ImageManager::new(picker, &settings, tx.clone());

    let mut app = App::new(
        setup.provider,
        settings,
        config_path.clone(),
        images,
        tx.clone(),
        initial_spec,
        initial_sort,
        notices,
    );
    // CLI flags are session overrides. Only in-app theme changes should update
    // the persisted theme; image settings have no in-app editor yet.
    app.persist_theme = config_theme;
    app.persist_images_enabled = config_images_enabled;
    app.persist_images_protocol = config_images_protocol;

    start_input_thread(tx.clone()).context("could not start the terminal input thread")?;
    start_ticker(tx.clone());

    let initial_size = terminal.size().context("could not read terminal size")?;
    app.set_size(initial_size.width, initial_size.height);
    app.prepare();
    terminal
        .draw(|frame| reddtty::ui::render(frame, &mut app))
        .context("could not draw the first frame")?;
    app.dirty = false;

    run_loop(&mut terminal, &mut app, rx).await?;

    // Persist changes made through explicit settings actions. The CLI flags
    // themselves are session overrides; there is no implicit save on exit.
    let _ = stdout().flush();
    Ok(())
}

async fn run_loop(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    mut rx: UnboundedReceiver<AppEvent>,
) -> Result<()> {
    let mut last_title = String::new();
    loop {
        let Some(event) = rx.recv().await else {
            break;
        };
        app.handle_event(event);
        let title = reddtty::utils::text::ellipsize(
            &reddtty::utils::text::sanitize(&format!("ReddTTY — {}", app.breadcrumb())),
            120,
        );
        if title != last_title {
            let _ = execute!(stdout(), SetTitle(title.clone()));
            last_title = title;
        }
        if app.should_quit {
            break;
        }
        if !app.dirty {
            continue;
        }
        let size = terminal.size().context("could not read terminal size")?;
        if app.force_clear {
            // Terminal::clear() queries the cursor, which races with the input
            // thread. resize() clears the viewport and resets the back buffer
            // without reading from stdin.
            terminal
                .resize(ratatui::layout::Rect::new(0, 0, size.width, size.height))
                .context("could not clear the terminal")?;
            app.force_clear = false;
        }
        app.set_size(size.width, size.height);
        app.prepare();
        terminal
            .draw(|frame| reddtty::ui::render(frame, app))
            .context("could not draw the interface")?;
        app.dirty = false;
    }

    // A last explicit save is safe: persist() only writes settings that the
    // application already owns. It also reports failures as a toast, which is
    // harmless on the way out.
    app.persist();
    terminal.show_cursor().ok();
    Ok(())
}

struct SessionGuard {
    mouse: bool,
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        tracing::info!("restoring terminal");
        if self.mouse {
            let _ = execute!(stdout(), DisableMouseCapture);
        }
        let _ = execute!(stdout(), Show, SetTitle("Terminal"));
        match ratatui::try_restore() {
            Ok(()) => tracing::info!("terminal restored"),
            Err(error) => tracing::error!(error = %error, "terminal restore failed"),
        }
        // Let the terminal process the alternate-screen switch before the
        // shell writes its next prompt/output. This avoids lost output in
        // multiplexers that batch escape sequences.
        std::thread::sleep(Duration::from_millis(40));
        let _ = writeln!(stdout());
    }
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = execute!(stdout(), DisableMouseCapture, Show);
        tracing::error!(panic = %info, "reddtty panicked");
        previous(info);
    }));
}

fn in_multiplexer() -> bool {
    std::env::var_os("TMUX").is_some()
        || std::env::var("TERM")
            .map(|term| term.starts_with("tmux") || term.starts_with("screen"))
            .unwrap_or(false)
}

fn make_picker(settings: &Settings, notices: &mut Vec<String>) -> Picker {
    if !settings.images.enabled || settings.images.protocol == ImageProtocolSetting::Disabled {
        return Picker::halfblocks();
    }

    // In a multiplexer without verified graphics passthrough, the stdio probe
    // only delays startup and can leave partial response bytes that swallow the
    // first keypress. Halfblocks are the documented safe fallback; a user who
    // has configured Kitty/Sixel passthrough can force the protocol.
    if matches!(
        settings.images.protocol,
        ImageProtocolSetting::Auto | ImageProtocolSetting::Halfblocks
    ) && in_multiplexer()
    {
        tracing::info!("multiplexer detected; using Unicode halfblocks without probing");
        return Picker::halfblocks();
    }

    let options = QueryStdioOptions {
        timeout: Duration::from_millis(300),
        ..Default::default()
    };
    tracing::info!("probing terminal image capabilities");
    let mut picker = match Picker::from_query_stdio_with_options(options) {
        Ok(picker) => picker,
        Err(error) => {
            notices.push(format!(
                "Could not query the terminal for image support ({error}); using Unicode halfblocks."
            ));
            Picker::halfblocks()
        }
    };

    match settings.images.protocol {
        ImageProtocolSetting::Auto => {
            // iTerm2 answers the Kitty probe, but both Kitty placeholders and
            // its inline-image path can be erased by full-screen Ratatui
            // repaints. Half-blocks are the reliable default there; users can
            // still force `iterm2`/`kitty` explicitly.
            let term_program = std::env::var("TERM_PROGRAM").ok();
            let lc_terminal = std::env::var("LC_TERMINAL").ok();
            let iterm_session = std::env::var_os("ITERM_SESSION_ID").is_some();
            if reddtty::media::is_iterm2_terminal(
                term_program.as_deref(),
                lc_terminal.as_deref(),
                iterm_session,
            ) {
                tracing::info!("iTerm2 detected; using Unicode half-blocks for reliable images");
                picker.set_protocol_type(ProtocolType::Halfblocks);
            }
        }
        ImageProtocolSetting::Kitty => picker.set_protocol_type(ProtocolType::Kitty),
        ImageProtocolSetting::Iterm2 => picker.set_protocol_type(ProtocolType::Iterm2),
        ImageProtocolSetting::Sixel => picker.set_protocol_type(ProtocolType::Sixel),
        ImageProtocolSetting::Halfblocks => picker.set_protocol_type(ProtocolType::Halfblocks),
        ImageProtocolSetting::Disabled => {}
    }
    tracing::info!(protocol = ?picker.protocol_type(), "terminal image capabilities probed");
    if settings.images.protocol != ImageProtocolSetting::Auto {
        notices.push(format!(
            "Image protocol forced to {}. If rendering looks broken, use protocol = \"auto\".",
            settings.images.protocol.label()
        ));
    }
    picker
}

fn start_input_thread(tx: UnboundedSender<AppEvent>) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name("reddtty-input".into())
        .spawn(move || {
            loop {
                match crossterm::event::read() {
                    Ok(Event::Key(key)) => {
                        if tx.send(AppEvent::Key(key)).is_err() {
                            break;
                        }
                    }
                    Ok(Event::Mouse(mouse)) => {
                        if tx.send(AppEvent::Mouse(mouse)).is_err() {
                            break;
                        }
                    }
                    Ok(Event::Resize(width, height)) => {
                        if tx.send(AppEvent::Resize(width, height)).is_err() {
                            break;
                        }
                    }
                    Ok(Event::Paste(text)) => {
                        if tx.send(AppEvent::Paste(text)).is_err() {
                            break;
                        }
                    }
                    Ok(Event::FocusGained | Event::FocusLost) => {}
                    Err(error) => {
                        tracing::warn!(error = %error, "terminal event reader stopped");
                        break;
                    }
                }
            }
        })
        .map(|_| ())
}

fn start_ticker(tx: UnboundedSender<AppEvent>) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(250));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            if tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });
}

fn init_logging() {
    let dir = Settings::cache_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("reddtty.log");
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok();
    if let Some(file) = &file {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        let _ = file;
    }
    let writer: BoxMakeWriter = match file {
        Some(file) => BoxMakeWriter::new(Mutex::new(file)),
        None => BoxMakeWriter::new(std::io::stderr),
    };
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("reddtty=info,warn"));
    let _ = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(filter)
        .try_init();
}

fn print_diagnostics(
    settings: &Settings,
    cli: &Cli,
    effective_config: &std::path::Path,
    initial_spec: &Option<FeedSpec>,
    warnings: &[String],
) {
    let credentials = settings.reddit_credentials();
    let mode = match settings.provider.kind {
        ProviderKind::Demo => "demo (explicit)",
        ProviderKind::Auto if credentials.is_some() => "live OAuth (auto-detected credentials)",
        ProviderKind::Auto => "demo fallback (auto; no Reddit credentials)",
        ProviderKind::Reddit => "Reddit OAuth (explicit)",
        ProviderKind::OldReddit => "Old Reddit HTML",
        ProviderKind::Redlib => "Redlib HTML",
    };

    println!("reddtty {}", env!("CARGO_PKG_VERSION"));
    println!(
        "platform: {} / {}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    println!(
        "terminal: TERM={} TERM_PROGRAM={} COLORTERM={}",
        std::env::var("TERM").unwrap_or_else(|_| "unset".into()),
        std::env::var("TERM_PROGRAM").unwrap_or_else(|_| "unset".into()),
        std::env::var("COLORTERM").unwrap_or_else(|_| "unset".into())
    );
    println!("stdout is a terminal: {}", stdout().is_terminal());
    println!(
        "config: {}",
        reddtty::utils::text::sanitize(&effective_config.display().to_string())
    );
    println!(
        "settings: {}",
        reddtty::utils::text::sanitize(&settings.redacted_summary())
    );
    println!(
        "reddit: credentials={} refresh_token={} user_agent_set={}",
        credentials.is_some(),
        credentials
            .as_ref()
            .is_some_and(|c| c.refresh_token.is_some()),
        !settings.reddit.user_agent.trim().is_empty()
    );
    println!("provider mode: {mode}");
    println!(
        "html provider: allow_network={} redlib_instance={}",
        settings.provider.html.allow_network,
        if settings.provider.html.redlib_instance.is_empty() {
            "unset".to_string()
        } else {
            reddtty::utils::text::sanitize(&settings.provider.html.redlib_instance)
        }
    );
    println!(
        "initial view: {}",
        initial_spec
            .as_ref()
            .map(FeedSpec::title)
            .unwrap_or_else(|| settings.general.default_feed.clone())
    );
    println!("secret values are never printed");

    if settings.images.enabled && stdout().is_terminal() {
        let options = QueryStdioOptions {
            timeout: Duration::from_millis(500),
            ..Default::default()
        };
        match Picker::from_query_stdio_with_options(options) {
            Ok(picker) => {
                let font = picker.font_size();
                let detected = picker.protocol_type();
                let term_program = std::env::var("TERM_PROGRAM").ok();
                let lc_terminal = std::env::var("LC_TERMINAL").ok();
                let iterm_session = std::env::var_os("ITERM_SESSION_ID").is_some();
                let effective = if settings.images.protocol == ImageProtocolSetting::Auto
                    && reddtty::media::is_iterm2_terminal(
                        term_program.as_deref(),
                        lc_terminal.as_deref(),
                        iterm_session,
                    ) {
                    ProtocolType::Halfblocks
                } else {
                    detected
                };
                if effective != detected {
                    println!(
                        "image protocol detected: {detected:?}; using {effective:?} (iTerm2 TUI fallback) (font {}x{} px, configured: {})",
                        font.width,
                        font.height,
                        settings.images.protocol.label()
                    );
                } else {
                    println!(
                        "image protocol detected: {effective:?} (font {}x{} px, configured: {})",
                        font.width,
                        font.height,
                        settings.images.protocol.label()
                    );
                }
            }
            Err(error) => println!("image protocol detection failed: {error}"),
        }
    } else {
        println!(
            "image protocol: not queried (images disabled or stdout is not a terminal); configured: {}",
            settings.images.protocol.label()
        );
    }

    if !warnings.is_empty() {
        println!("config warnings:");
        for warning in warnings {
            println!("  - {}", reddtty::utils::text::sanitize(warning));
        }
    }
    println!(
        "notes: Reddit OAuth access is sunsetting; HTML providers are not the official API and require provider.html.allow_network plus your own terms/permission check. No live access is claimed without a successful run. See docs/API_ACCESS.md."
    );
    let _ = cli;
}
