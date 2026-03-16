use std::sync::mpsc;

use terrar::app;
use terrar::sources;

/// Spawn all data source polling loops. Works on both native (tokio) and WASM (wasm-bindgen-futures).
fn spawn_sources(tx: mpsc::Sender<Vec<sources::ActorUpdate>>) {
    // ISS — every 5s
    let tx_iss = tx.clone();
    spawn(async move {
        loop {
            if let Some(updates) = sources::iss::fetch().await {
                let _ = tx_iss.send(updates);
            }
            sleep_secs(5).await;
        }
    });

    // OpenSky — every 10s
    let tx_opensky = tx.clone();
    spawn(async move {
        loop {
            if let Some(updates) = sources::opensky::fetch((47.0, 5.0, 55.0, 15.0)).await {
                let _ = tx_opensky.send(updates);
            }
            sleep_secs(10).await;
        }
    });

    // Ships — every 15s
    let tx_ships = tx.clone();
    spawn(async move {
        loop {
            let mut all = Vec::new();
            if let Some(updates) = sources::ships::fetch(59.0, 22.0, 400.0).await {
                all.extend(updates);
            }
            if let Some(updates) = sources::ships::fetch(55.5, 14.0, 300.0).await {
                all.extend(updates);
            }
            if !all.is_empty() {
                let _ = tx_ships.send(all);
            }
            sleep_secs(15).await;
        }
    });

    // VBB Berlin transit — every 10s
    let tx_vbb = tx.clone();
    spawn(async move {
        loop {
            if let Some(updates) = sources::vbb::fetch().await {
                let _ = tx_vbb.send(updates);
            }
            sleep_secs(10).await;
        }
    });

    // Autobahn — cycles through roads in batches every 30s
    let tx_autobahn = tx.clone();
    spawn(async move {
        let roads = sources::autobahn::fetch_roads().await;
        if roads.is_empty() {
            log::warn!("No Autobahn roads found, skipping source");
            return;
        }
        let batch_size = 10;
        let mut offset = 0;
        loop {
            let batch: Vec<String> = roads
                .iter()
                .cycle()
                .skip(offset)
                .take(batch_size)
                .cloned()
                .collect();
            offset = (offset + batch_size) % roads.len();
            if let Some(updates) = sources::autobahn::fetch(&batch).await {
                let _ = tx_autobahn.send(updates);
            }
            sleep_secs(30).await;
        }
    });

    // DWD weather warnings — every 60s
    let tx_dwd = tx.clone();
    spawn(async move {
        loop {
            if let Some(updates) = sources::dwd::fetch().await {
                let _ = tx_dwd.send(updates);
            }
            sleep_secs(60).await;
        }
    });
}

// --- Platform-specific spawn/sleep ---

#[cfg(not(target_arch = "wasm32"))]
fn spawn(future: impl std::future::Future<Output = ()> + Send + 'static) {
    // Lazily create a tokio runtime that lives for the program's lifetime
    use std::sync::OnceLock;
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    let rt = RT.get_or_init(|| {
        tokio::runtime::Runtime::new().expect("Failed to create tokio runtime")
    });
    rt.spawn(future);
}

#[cfg(not(target_arch = "wasm32"))]
async fn sleep_secs(secs: u64) {
    tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
}

#[cfg(target_arch = "wasm32")]
fn spawn(future: impl std::future::Future<Output = ()> + 'static) {
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(target_arch = "wasm32")]
async fn sleep_secs(secs: u64) {
    gloo_timers::future::sleep(std::time::Duration::from_secs(secs)).await;
}

// --- Entry points ---

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    env_logger::init();

    let (tx, rx) = mpsc::channel();
    spawn_sources(tx);

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "terrar",
        native_options,
        Box::new(|cc| Ok(Box::new(app::TerrarApp::new(cc, rx)))),
    )
    .expect("Failed to run eframe");
}

#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast;
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let (tx, rx) = mpsc::channel();
    spawn_sources(tx);

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("no canvas element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("not a canvas");

        eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(app::TerrarApp::new(cc, rx)))),
            )
            .await
            .expect("failed to start eframe");
    });
}
