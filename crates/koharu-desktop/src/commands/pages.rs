//! The pages: the one the canvas shows, and the pages rail's thumbnails.

use std::sync::Arc;

use anyhow::{Context as _, ensure};
use koharu_scene::PageId;
use tauri::State;

use super::{Bytes, Result};
use crate::{app::App, events};

/// How tall a thumbnail is, in pixels.
const HEIGHT: u32 = 256;

/// The page as it renders, `HEIGHT` pixels tall, as WebP: drawn twice as large
/// and shrunk, as masks' edges are jagged drawn at that size.
#[tauri::command]
#[specta::specta]
pub async fn get_page_thumbnail(app: State<'_, Arc<App>>, page: PageId) -> Result<Bytes> {
    let app = Arc::clone(&app);
    Ok(tokio::task::spawn_blocking(move || {
        let session = app.session()?;
        let frame = app.renderer.render(&session.lock(), page)?;
        let area = frame.area();
        ensure!(!area.is_zero_area(), "the page is empty");
        let width = (area.width() * f64::from(HEIGHT) / area.height()).max(1.0) as u32;
        let drawn = frame.rasterize_scaled(width * 2, HEIGHT * 2)?;
        let image = image::imageops::thumbnail(&drawn, width, HEIGHT);
        let encoder = webp::Encoder::from_rgba(image.as_raw(), image.width(), image.height());
        Ok(Bytes(encoder.encode(80.0).to_vec()))
    })
    .await??)
}

/// The page the canvas shows.
#[tauri::command]
#[specta::specta]
pub fn get_shown_page(app: State<'_, Arc<App>>) -> Option<PageId> {
    app.project.lock().as_ref().and_then(|project| project.page)
}

/// Shows `page` on the canvas.
#[tauri::command]
#[specta::specta]
pub async fn show(app: State<'_, Arc<App>>, page: PageId) -> Result<()> {
    let app = Arc::clone(&app);
    // Typing on the page shown commits before the page changes.
    app.screen.finish_typing(Arc::clone(&app));
    Ok(tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let moved = {
            let mut project = app.project.lock();
            let project = project.as_mut().context("no project is open")?;
            let mut session = project.session.lock();
            session.project().page(page)?;
            let moved = project.page != Some(page);
            if moved {
                session.select_layers(page, Vec::new(), false);
            }
            project.page = Some(page);
            moved
        };
        app.emit(events::PAGE, Some(page));
        if moved {
            app.emit(events::SELECTION, Vec::new());
        }
        app.refresh()
    })
    .await??)
}
