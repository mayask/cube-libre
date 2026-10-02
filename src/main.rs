mod app;

fn main() {
    use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "cube_ble=info".into()),
        )
        .try_init();
    let backend = cube_ble::Backend::spawn();
    dioxus::LaunchBuilder::desktop()
        .with_context(backend)
        .with_cfg(
            Config::new()
                .with_window(
                    WindowBuilder::new()
                        .with_title("Cube Libre")
                        .with_inner_size(LogicalSize::new(1180.0, 790.0))
                        .with_min_inner_size(LogicalSize::new(820.0, 640.0)),
                )
                .with_icon(
                    dioxus::desktop::icon_from_memory(include_bytes!("../assets/icon.png"))
                        .expect("embedded app icon"),
                )
                .with_menu(None)
                .with_background_color((13, 17, 23, 255)),
        )
        .launch(app::App);
}
