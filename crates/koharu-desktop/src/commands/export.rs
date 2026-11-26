//! Export all or selected pages as PNG, PSD, or CBZ.

use std::{
    collections::HashSet,
    io::{Cursor, ErrorKind, Write as _},
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
    single_page: bool,
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
    let single = single_file(&formats, pages.is_some() && single_page && numbered.len() == 1);
    let destination = if let Some((label, extension)) = single {
        let filename = if formats == [ExportFormat::Cbz] {
            archive_name.clone()
        } else {
            format!("{}.{extension}", numbered[0].0)
        };
        dialog
            .set_file_name(filename)
            .add_filter(label, &[extension])
            .save_file()
            .await
    } else {
        dialog.pick_folder().await
    };
    let Some(mut destination) = destination.map(|picked| picked.path().to_owned()) else {
        return Ok(());
    };
    if let Some((_, extension)) = single
        && destination.extension().is_none()
    {
        destination.set_extension(extension);
    }
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
            single.is_some(),
        )
    })
    .await??)
}

/// Archives keep their existing save dialog; only the dedicated single-page
/// action names an individual image.
fn single_file(formats: &[ExportFormat], single_page: bool) -> Option<(&'static str, &'static str)> {
    match formats {
        [ExportFormat::Cbz] => Some(("CBZ", "cbz")),
        [ExportFormat::Png] if single_page => Some(("PNG", "png")),
        [ExportFormat::Psd] if single_page => Some(("PSD", "psd")),
        _ => None,
    }
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
    single: bool,
) -> anyhow::Result<()> {
    let export_png = formats.contains(&ExportFormat::Png);
    let export_psd = formats.contains(&ExportFormat::Psd);
    let mut archive = if formats.contains(&ExportFormat::Cbz) {
        let file = if single {
            File::create(destination)?
        } else {
            create_versioned(&destination.join(archive_name))?
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
                if single {
                    fs_err::write(destination, &data)?;
                } else {
                    create_versioned(&destination.join(&name))?.write_all(&data)?;
                }
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
            if single {
                fs_err::write(destination, psd)?;
            } else {
                create_versioned(&destination.join(format!("{number}.psd")))?.write_all(&psd)?;
            }
        }
    }
    if let Some(archive) = archive {
        archive.finish()?;
    }
    Ok(())
}

/// Reserve an unused name atomically, so even simultaneous exports cannot overwrite a file.
fn create_versioned(path: &Path) -> anyhow::Result<File> {
    let stem = path.file_stem().context("export path has no filename")?;
    let mut candidate = path.to_owned();
    let mut version = 1u64;
    loop {
        match fs_err::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok(file),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                let mut name = stem.to_os_string();
                name.push(format!("_v{version}"));
                if let Some(extension) = path.extension() {
                    name.push(".");
                    name.push(extension);
                }
                candidate = path.with_file_name(name);
                version += 1;
            }
            Err(error) => return Err(error.into()),
        }
    }
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
    fn only_the_single_page_action_names_individual_images() {
        assert_eq!(single_file(&[ExportFormat::Png], false), None);
        assert_eq!(single_file(&[ExportFormat::Psd], false), None);
        assert_eq!(single_file(&[ExportFormat::Png], true), Some(("PNG", "png")));
        assert_eq!(single_file(&[ExportFormat::Psd], true), Some(("PSD", "psd")));
        assert_eq!(single_file(&[ExportFormat::Cbz], false), Some(("CBZ", "cbz")));
        assert_eq!(single_file(&[ExportFormat::Png, ExportFormat::Psd], true), None);
    }

    #[test]
    fn exports_preserve_existing_files_and_use_the_next_free_version() -> anyhow::Result<()> {
        let dir = std::env::temp_dir().join(format!("kipoharu-export-{}", Uuid::new_v4()));
        fs_err::create_dir(&dir)?;
        let result = (|| -> anyhow::Result<()> {
            let path = dir.join("001.png");
            create_versioned(&path)?.write_all(b"original")?;
            create_versioned(&path)?.write_all(b"first correction")?;
            create_versioned(&path)?.write_all(b"second correction")?;
            assert_eq!(fs_err::read(&path)?, b"original");
            assert_eq!(fs_err::read(dir.join("001_v1.png"))?, b"first correction");
            assert_eq!(fs_err::read(dir.join("001_v2.png"))?, b"second correction");
            assert!(create_versioned(&dir.join("missing/001.png")).is_err());
            Ok(())
        })();
        fs_err::remove_dir_all(&dir)?;
        result
    }

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
