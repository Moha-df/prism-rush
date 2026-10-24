//! Prism Rush — a cooperative roguelike FPS (1-4 players) built with Bevy.
//!
//! Entry point: parses the command line, then assembles the Bevy `App` from
//! plugins. Run `prism_rush --help` for all modes.

use std::time::Duration;

use bevy::app::ScheduleRunnerPlugin;
use bevy::log::LogPlugin;
use bevy::prelude::*;
use bevy::state::app::StatesPlugin;
use bevy::window::{PresentMode, WindowLevel, WindowResolution};
use bevy_replicon::prelude::*;
use bevy_replicon_iroh::{IrohTokioHandle, RepliconIrohPlugins};
use clap::{Parser, Subcommand};

use prism_rush::config::{AppConfig, LaunchMode};
use prism_rush::{bot, game, net, render, shared, ui};

#[derive(Parser, Debug)]
#[command(name = "prism_rush", version, about = "FPS roguelike coopératif (1-4 joueurs)")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Player name (otherwise the saved one is used).
    #[arg(long, global = true)]
    name: Option<String>,
    /// UDP port.
    #[arg(long, global = true, default_value_t = net::protocol::DEFAULT_PORT)]
    port: u16,
    /// Debug: take screenshots into this folder at fixed times, then quit.
    #[arg(long, global = true)]
    shots: Option<String>,
    /// Debug: let a bot drive your player.
    #[arg(long, global = true)]
    autopilot: bool,
    /// Quit after this many seconds (automated tests).
    #[arg(long, global = true)]
    duration: Option<f32>,
    /// Debug: start runs at this time in seconds (e.g. 900 = Final Swarm).
    #[arg(long, global = true, default_value_t = 0.0)]
    start_time: f32,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Opens the main menu (default).
    Menu,
    /// Hosts a game (you play + others can join).
    Host {
        /// Number of AI bots to add.
        #[arg(long, default_value_t = 0)]
        bots: u8,
    },
    /// Joins a game by its code (shown in the host's lobby).
    Join { code: String },
    /// Solo game (host without waiting in the lobby).
    Solo {
        #[arg(long, default_value_t = 0)]
        bots: u8,
    },
    /// Dedicated server. Use --headless for no window (tests, servers).
    Server {
        #[arg(long)]
        headless: bool,
        #[arg(long, default_value_t = 0)]
        bots: u8,
        /// Start the run immediately instead of waiting in the lobby.
        #[arg(long)]
        autostart: bool,
        #[arg(long)]
        seed: Option<u64>,
    },
    /// Headless client controlled by a bot (network test). Without a code,
    /// joins the first game of the public list.
    Bot {
        #[arg(default_value = "")]
        code: String,
    },
}

/// Folder for log files, next to the executable (found even when launched
/// from another folder or by double-click).
fn log_dir() -> std::path::PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("logs")))
        .unwrap_or_else(|| std::path::PathBuf::from("logs"))
}

/// If the game ever crashes, write the reason to `logs/crash.log`.
fn install_crash_log() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let text = format!("Prism Rush a plante :
{info}

{backtrace}
");
        let dir = log_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("crash.log"), text);
        default_hook(info);
    }));
}

/// Extra log output: everything that is printed in the console is also
/// written to `logs/prism_rush.log` (the previous run is kept as
/// `prism_rush.prev.log`). Plain function (no closure) because `LogPlugin`
/// expects a function pointer.
fn file_log_layer(_app: &mut App) -> Option<bevy::log::BoxedLayer> {
    use bevy::log::tracing_subscriber::{self, Layer};
    let dir = log_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("prism_rush.log");
    let _ = std::fs::rename(&path, dir.join("prism_rush.prev.log"));
    let file = std::fs::File::create(&path).ok()?;
    let layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(std::sync::Mutex::new(file))
        .with_filter(tracing_subscriber::filter::LevelFilter::INFO);
    Some(Box::new(layer))
}

/// Logs why the app is closing (normal exit vs. error), to tell apart a
/// requested exit from a hard crash in the log file.
fn log_app_exit(mut exits: MessageReader<AppExit>, closed: Option<MessageReader<bevy::window::WindowCloseRequested>>) {
    // `Option`: the window message doesn't exist in headless mode.
    if let Some(mut closed) = closed {
        for e in closed.read() {
            info!("window close requested ({:?})", e.window);
        }
    }
    for e in exits.read() {
        warn!("application exit requested: {e:?}");
    }
}

fn main() {
    let cli = Cli::parse();
    let mut config = AppConfig::load();
    if let Some(name) = cli.name {
        config.name = name;
    }
    config.port = cli.port;
    config.shots = cli.shots;
    config.autopilot = cli.autopilot;
    config.duration = cli.duration;
    config.start_time = cli.start_time;

    config.launch = match cli.command.unwrap_or(Command::Menu) {
        Command::Menu => LaunchMode::Menu,
        Command::Host { bots } => {
            config.bots = bots;
            LaunchMode::Host
        }
        Command::Join { code } => {
            config.last_address = code;
            LaunchMode::Join
        }
        Command::Solo { bots } => {
            config.bots = bots;
            config.autostart = true;
            config.solo = true;
            LaunchMode::Host
        }
        Command::Server { headless, bots, autostart, seed } => {
            config.headless = headless;
            config.bots = bots;
            config.autostart = autostart;
            config.seed = seed;
            LaunchMode::Dedicated
        }
        Command::Bot { code } => {
            config.last_address = code;
            config.headless = true;
            config.name = format!("Bot-{}", std::process::id() % 1000);
            LaunchMode::BotClient
        }
    };

    install_crash_log();

    let mut app = App::new();
    // By default Bevy *closes the game* when a command targets an entity that
    // no longer exists (e.g. a monster killed twice in the same tick). In a
    // networked game such races are normal: log a warning and keep playing.
    app.set_error_handler(bevy::ecs::error::warn);
    if config.headless {
        // No window, no GPU: just the simulation loop at ~120 Hz.
        app.add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(1.0 / 120.0))),
            StatesPlugin,
            LogPlugin { custom_layer: file_log_layer, ..default() },
            TransformPlugin,
        ));
    } else {
        app.add_plugins(DefaultPlugins.set(LogPlugin { custom_layer: file_log_layer, ..default() }).set(WindowPlugin {
            primary_window: Some(Window {
                title: "Prism Rush".into(),
                resolution: WindowResolution::new(1600, 900),
                present_mode: PresentMode::AutoVsync,
                // Automated visual tests must not steal the keyboard/mouse.
                window_level: if config.shots.is_some() || config.autopilot {
                    WindowLevel::AlwaysOnBottom
                } else {
                    WindowLevel::Normal
                },
                focused: config.shots.is_none() && !config.autopilot,
                ..default()
            }),
            ..default()
        }));
    }

    // iroh (the network library) is asynchronous and runs on a tokio runtime.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    app.insert_resource(IrohTokioHandle(runtime.handle().clone()))
        .insert_resource(net::TokioRuntime(runtime));
    app.add_plugins((RepliconPlugins, RepliconIrohPlugins))
        // Same fixed tick everywhere: 60 Hz simulation + replication.
        .insert_resource(Time::<Fixed>::from_hz(shared::movement::TICK_HZ));
    net::protocol::register_protocol(&mut app);

    let headless = config.headless;
    app.insert_resource(config)
        .add_plugins((net::NetPlugin, game::GamePlugin, bot::BotPlugin));
    if !headless {
        app.add_plugins((render::RenderPlugin, ui::UiPlugin));
    }
    app.add_systems(Last, log_app_exit);
    info!("Prism Rush {} — logs dans {}", env!("CARGO_PKG_VERSION"), log_dir().display());
    app.run();
}
