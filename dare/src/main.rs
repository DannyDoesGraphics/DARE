use dare_engine::{EnginePluginConfig, bootstrap_engine};
use dare_render::{RenderPlugin, RenderPluginConfig};
use tracing_subscriber::FmtSubscriber;

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
        .with_max_level(tracing::Level::INFO)
        .with_file(true)
        .with_line_number(true)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let _client = tracy_client::Client::start();

    let mut app = bootstrap_engine(EnginePluginConfig {
        prompt_gltf_on_startup: true,
        ..Default::default()
    });
    app.add_plugin(RenderPlugin::new(RenderPluginConfig::default()));

    app.run();
}
