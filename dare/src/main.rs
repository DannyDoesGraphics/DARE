use dare_engine::{EnginePluginConfig, bootstrap_engine};
use dare_render::{RenderPlugin, RenderPluginConfig};
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

fn parse_gltf_arg() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--gltf" {
            return args.next().map(PathBuf::from);
        }
        if let Some(path) = arg.strip_prefix("--gltf=") {
            return Some(PathBuf::from(path));
        }
    }
    None
}

fn main() {
    tracy_client::Client::start();
    std::panic::set_hook(Box::new(|info| {
        use std::io::Write;
        eprintln!("The program panicked: {}", info);
        print!("Press Enter to exit...");
        std::io::stdout().flush().expect("Failed to flush stdout");
        let _ = std::io::stdin().read_line(&mut String::new());
    }));

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .with_file(true)
        .with_line_number(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let _client = tracy_client::Client::start();

    let gltf_path = parse_gltf_arg();
    let mut app = bootstrap_engine(EnginePluginConfig {
        prompt_gltf_on_startup: gltf_path.is_none(),
        initial_gltf: gltf_path.into_iter().collect(),
        ..Default::default()
    });
    app.add_plugin(RenderPlugin::new(RenderPluginConfig::default()));
    app.add_plugin(dare_ecs::SmolPlugin);

    app.run();
}
