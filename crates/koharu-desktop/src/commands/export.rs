//! Export all or selected pages as PNG, PSD, or CBZ.

use std::{
    collections::HashSet,
    io::{Cursor, Write as _},
    path::Path,
    sync::Arc,
};

use anyhow::{Context as _, ensure};
use fs_err::File;
use image::ImageFormat;
use koharu_psd::Labels;
use koharu_renderer::Frame;
use koharu_scene::{PageId, Session};
use parking_lot::Mutex;
use serde::Deserialize;
use specta::Type;
use tauri::{State, Window};
use tauri_runtime_cef::CefRuntime;

use super::Result;
use crate::app::App;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Png,
    Psd,
    Cbz,
}

/// Picks a destination and exports the requested formats. `pages` is null for the entire
/// project; otherwise only the named pages are written, in project order. `labels` names a
/// PSD's unnamed layers in the interface's language.
#[tauri::command]
#[specta::specta]
pub async fn export(
    app: State<'_, Arc<App>>,
    window: Window<CefRuntime>,
    formats: Vec<ExportFormat>,
    labels: Labels,
    pages: Option<Vec<PageId>>,
) -> Result<()> {
    if formats.is_empty() {
        return Err(anyhow::anyhow!("no export format selected").into());
    }
    let (name, session) = app
        .project
        .lock()
        .as_ref()
        .map(|project| (project.name.clone(), Arc::clone(&project.session)))
        .context("no project is open")?;
    let all: Vec<PageId> = session.lock().project().pages().map(|page| page.id).collect();
    let numbered = numbered_pages(&all, pages.as_deref())?;
    let archive_name = match pages {
        None => format!("{name}.cbz"),
        Some(_) if numbered.len() == 1 => format!("{name} - {}.cbz", numbered[0].0),
        Some(_) => format!("{name} - selected.cbz"),
    };
    let dialog = rfd::AsyncFileDialog::new().set_parent(&window);
    let destination = if formats == [ExportFormat::Cbz] {
        dialog
            .set_file_name(archive_name.clone())
            .add_filter("CBZ", &["cbz"])
            .save_file()
            .await
    } else {
        dialog.pick_folder().await
    };
    let Some(destination) = destination.map(|picked| picked.path().to_owned()) else {
        return Ok(());
    };
    let app = Arc::clone(&app);
    Ok(tokio::task::spawn_blocking(move || {
        write(
            &app,
            &session,
            &formats,
            &labels,
            &destination,
            &archive_name,
            &numbered,
        )
    })
    .await??)
}

/// Filter in project order, retaining the original page numbers even for a subset.
fn numbered_pages(
    all: &[PageId],
    selected: Option<&[PageId]>,
) -> anyhow::Result<Vec<(String, PageId)>> {
    let selected = selected.map(|ids| ids.iter().copied().collect::<HashSet<_>>());
    if let Some(ids) = &selected {
        ensure!(!ids.is_empty(), "no pages selected for export");
        ensure!(
            ids.iter().all(|id| all.contains(id)),
            "an export page is no longer in the project"
        );
    }
    let digits = all.len().to_string().len().max(3);
    let pages: Vec<_> = all
        .iter()
        .enumerate()
        .filter(|(_, page)| selected.as_ref().is_none_or(|ids| ids.contains(*page)))
        .map(|(index, &page)| (format!("{:0digits$}", index + 1), page))
        .collect();
    ensure!(!pages.is_empty(), "no pages to export");
    Ok(pages)
}

fn write(
    app: &App,
    session: &Mutex<Session>,
    formats: &[ExportFormat],
    labels: &Labels,
    destination: &Path,
    archive_name: &str,
    pages: &[(String, PageId)],
) -> anyhow::Result<()> {
    let export_png = formats.contains(&ExportFormat::Png);
    let export_psd = formats.contains(&ExportFormat::Psd);
    let mut archive = if formats.contains(&ExportFormat::Cbz) {
        let file = if formats == [ExportFormat::Cbz] {
            File::create(destination)?
        } else {
            // The folder picker has no overwrite confirmation for the archive inside it.
            fs_err::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(destination.join(archive_name))?
        };
        Some(::zip::ZipWriter::new(file))
    } else {
        None
    };
    for (number, page) in pages {
        if export_png || archive.is_some() {
            let frame = app.renderer.render(&session.lock(), *page)?;
            let data = png(&frame)?;
            let name = format!("{number}.png");
            if export_png {
                fs_err::write(destination.join(&name), &data)?;
            }
            if let Some(archive) = &mut archive {
                // Already-compressed PNGs gain nothing from deflate.
                let options = ::zip::write::SimpleFileOptions::default()
                    .compression_method(::zip::CompressionMethod::Stored);
                archive.start_file(name, options)?;
                archive.write_all(&data)?;
            }
        }
        if export_psd {
            let psd = koharu_psd::export(&session.lock(), &app.renderer, *page, labels)?;
            fs_err::write(destination.join(format!("{number}.psd")), psd)?;
        }
    }
    if let Some(archive) = archive {
        archive.finish()?;
    }
    Ok(())
}

fn png(frame: &Frame) -> anyhow::Result<Vec<u8>> {
    let raster = frame.rasterize()?;
    let mut png = Cursor::new(Vec::new());
    raster.image.write_to(&mut png, ImageFormat::Png)?;
    Ok(png.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use koharu_scene::Uuid;

    #[test]
    fn subsets_keep_project_order_and_numbers() {
        let pages = [1, 2, 3].map(|id| PageId(Uuid::from_u128(id)));
        let selected = [pages[2], pages[0], pages[2]];
        assert_eq!(
            numbered_pages(&pages, Some(&selected)).unwrap(),
            vec![("001".to_owned(), pages[0]), ("003".to_owned(), pages[2])]
        );
        assert_eq!(numbered_pages(&pages, None).unwrap().len(), 3);
        assert!(numbered_pages(&pages, Some(&[])).is_err());
        assert!(numbered_pages(&pages, Some(&[PageId(Uuid::from_u128(4))])).is_err());
    }
}
