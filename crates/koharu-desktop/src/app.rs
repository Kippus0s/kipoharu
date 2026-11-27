//! The app's state: the projects folder, the open project and what runs on
//! it, the renderer and the pipeline, and the window. The open project's
//! session tells every change, whichever writer made it, and the app shows
//! and tells each.

use std::{
    collections::BTreeSet,
    sync::{Arc, OnceLock},
};

use anyhow::{Context as _, Result};
use futures::StreamExt as _;
use koharu_canvas::{Canvas, Effect, Snapshot};
use koharu_pipeline::{Area, Pipeline, PipelineConfig, Request, RunId, Stage, Target};
use koharu_renderer::Renderer;
use koharu_scene::{Change, Created, Edit, LayerId, PageId, Session};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Manager as _};
use tauri_runtime_cef::CefRuntime;
use tokio::sync::broadcast::{Receiver, error::RecvError};
use tokio_util::task::AbortOnDropHandle;

use crate::{
    events::{self, Event},
    library::Library,
    screen::{self, Screen},
};

pub struct App {
    pub library: Library,
    pub renderer: Renderer,
    pub pipeline: Pipeline,
    pub screen: Screen,
    pub project: Mutex<Option<Project>>,
    /// Set once the window's app starts, before anything is told.
    handle: OnceLock<AppHandle<CefRuntime>>,
}

/// The open project, and what runs on it, which go when it closes.
pub struct Project {
    pub name: String,
    pub session: Arc<Mutex<Session>>,
    /// The page the canvas shows; `None` when the project has none.
    pub page: Option<PageId>,
    /// Shows and tells the session's changes.
    _relay: AbortOnDropHandle<()>,
}

impl App {
    pub fn new(library: Library, renderer: Renderer, pipeline: Pipeline) -> Self {
        Self {
            library,
            renderer,
            screen: Screen::default(),
            pipeline,
            project: Mutex::new(None),
            handle: OnceLock::new(),
        }
    }

    /// Starts telling the interface every run's events and every download.
    pub fn start(self: &Arc<Self>, handle: AppHandle<CefRuntime>) {
        let _ = self.handle.set(handle);
        let app = Arc::clone(self);
        let mut runs = self.pipeline.events();
        tauri::async_runtime::spawn(async move {
            loop {
                match runs.recv().await {
                    Ok(event) => app.emit(events::PIPELINE, event),
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => return,
                }
            }
        });
        let app = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let mut transfers = koharu_runtime::transfers();
            while let Some(transfer) = transfers.next().await {
                app.emit(events::DOWNLOAD, transfer);
            }
        });
    }

    /// Tells the interface `payload`, once it can be told.
    pub fn emit<T: Serialize + Clone>(&self, event: Event<T>, payload: T) {
        if let Some(handle) = self.handle.get() {
            event.tell(handle, payload);
        }
    }

    /// Brings the window forward, for Koharu launched again while it runs.
    pub fn bring_forward(&self) {
        if let Some(window) = self
            .handle
            .get()
            .and_then(|handle| handle.get_window(screen::MAIN))
        {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }

    /// Opens `session` in place of the open project, at its first page.
    pub fn open(self: &Arc<Self>, name: String, session: Session) -> Result<()> {
        self.close();
        let page = session.project().pages().next().map(|page| page.id);
        let relay = self.relay(session.subscribe());
        let session = Arc::new(Mutex::new(session));
        self.screen.set_canvas(Some(Canvas::new(
            Arc::clone(&session),
            self.renderer.clone(),
        )));
        *self.project.lock() = Some(Project {
            name: name.clone(),
            session,
            page,
            _relay: relay,
        });
        self.emit(events::PROJECT, Some(name));
        self.emit(events::PAGE, page);
        self.refresh()
    }

    /// Closes the open project, if one is, and stops what runs on it, so
    /// nothing writes to a closed session.
    pub fn close(&self) {
        let project = self.project.lock().take();
        self.pipeline.stop();
        self.screen.set_canvas(None);
        if let Some(project) = project {
            self.pipeline.close(project.session.lock().id());
            self.emit(events::PROJECT, None);
            self.emit(events::PAGE, None);
        }
    }

    /// The open project's session.
    pub fn session(&self) -> Result<Arc<Mutex<Session>>> {
        self.project
            .lock()
            .as_ref()
            .map(|project| Arc::clone(&project.session))
            .context("no project is open")
    }

    /// Runs `stages` on `target` in the open project, with `config` in the
    /// pipeline's settings' place when given. The pipeline traces the run
    /// under whatever asked for it.
    pub fn process(
        &self,
        target: Target,
        stages: BTreeSet<Stage>,
        config: Option<PipelineConfig>,
    ) -> Result<RunId> {
        Ok(self.pipeline.run(
            self.session()?,
            Request {
                target,
                stages,
                config,
            },
        )?)
    }

    /// Runs `edit` on the open project as one undo step.
    pub fn edit(&self, edit: Edit) -> Result<Change> {
        tracing::info!(label = %edit.label, ops = edit.ops.len(), "edit");
        let session = self.session()?;
        let mut session = session.lock();
        let edit = self
            .renderer
            .prepare_manual_text_edit(&session.project(), edit)?;
        Ok(session.apply(edit)?)
    }

    /// Tells the interface `session`'s changes, and shows them on the canvas
    /// one redraw at a time: a burst, and what comes during a redraw, shows
    /// once. The shown page settles before the interface hears of a change,
    /// as it reads the shown page again for one: a page the change removed
    /// would otherwise be the one it asks for.
    fn relay(self: &Arc<Self>, mut changes: Receiver<Change>) -> AbortOnDropHandle<()> {
        let app = Arc::clone(self);
        let diagnostics = std::env::var("KIPOHARU_HISTORY_DIAGNOSTICS").as_deref() == Ok("1");
        AbortOnDropHandle::new(tokio::spawn(async move {
            loop {
                let mut told = Vec::new();
                let lagged = match changes.recv().await {
                    Ok(change) => {
                        told.push(change);
                        false
                    }
                    Err(RecvError::Lagged(_)) => true,
                    Err(RecvError::Closed) => return,
                };
                while let Ok(change) = changes.try_recv() {
                    told.push(change);
                }
                let showing = Arc::clone(&app);
                let shown = tokio::task::spawn_blocking(move || {
                    let (restored, changed_page) = showing.history_selection();
                    let redraw = lagged
                        || changed_page
                        || told.iter().any(|change| {
                            change.pages != Default::default() || !change.layers.is_empty()
                        });
                    let shown = showing.settle();
                    // The interface missed changes: it reads the project again.
                    if lagged {
                        let name = showing
                            .project
                            .lock()
                            .as_ref()
                            .map(|open| open.name.clone());
                        showing.emit(events::PROJECT, name);
                    }
                    // The Remove tool creates this labelled raster asynchronously. Select
                    // it only once the edit has succeeded and the canvas has its new page.
                    let created = told
                        .iter()
                        .rev()
                        .flat_map(|change| &change.created)
                        .find_map(|made| match made {
                            Created::Layer {
                                id,
                                label: Some(label),
                            } if label == "inpainted" => Some(*id),
                            _ => None,
                        });
                    for change in told {
                        showing.emit(events::CHANGE, change);
                    }
                    if diagnostics && let Some((session, _)) = &shown {
                        match session.lock().history_storage() {
                            Ok(stats) => tracing::info!(?stats, "history storage"),
                            Err(error) => tracing::warn!(%error, "history storage diagnostic failed"),
                        }
                    }
                    let result = if redraw {
                        shown.map_or(Ok(()), |shown| showing.show(shown))
                    } else {
                        Ok(())
                    };
                    if let Some((page, layers)) = restored.filter(|_| result.is_ok()) {
                        showing.screen.restore_selection(Arc::clone(&showing), page, layers);
                    } else if let Some(layer) = created.filter(|_| result.is_ok()) {
                        let on_page = showing.project.lock().as_ref().is_some_and(|project| {
                            project.page.is_some_and(|page| {
                                project
                                    .session
                                    .lock()
                                    .project()
                                    .parent(layer)
                                    .is_ok_and(|(owner, _)| owner.id == page)
                            })
                        });
                        if on_page {
                            showing
                                .screen
                                .select_and_notify(Arc::clone(&showing), vec![layer]);
                        }
                    }
                    result
                })
                .await;
                if let Err(error) = shown.map_err(anyhow::Error::from).and_then(|result| result) {
                    tracing::error!("failed to show the project: {error:#}");
                }
            }
        }))
    }

    /// A history jump can restore a layer selection on another page. It is
    /// transient and must not create a new selection step.
    fn history_selection(&self) -> (Option<(PageId, Vec<LayerId>)>, bool) {
        let (restored, moved) = {
            let mut open = self.project.lock();
            let Some(open) = open.as_mut() else {
                return (None, false);
            };
            let mut session = open.session.lock();
            let restored = session.take_restored_selection()
                .filter(|(page, _)| session.project().page(*page).is_ok());
            let moved = restored.as_ref().is_some_and(|(page, _)| open.page != Some(*page));
            if let Some((page, _)) = &restored {
                open.page = Some(*page);
            }
            (restored, moved)
        };
        if moved {
            self.emit(events::PAGE, restored.as_ref().map(|(page, _)| *page));
        }
        (restored, moved)
    }

    /// Shows the open project's page as it is now. A page that went away
    /// gives its place to the first.
    pub fn refresh(self: &Arc<Self>) -> Result<()> {
        self.settle().map_or(Ok(()), |shown| self.show(shown))
    }

    /// The open project's session and the page it shows, which moves to the
    /// first when it went away, telling the interface so.
    fn settle(&self) -> Option<(Arc<Mutex<Session>>, Option<PageId>)> {
        let (session, page, moved) = {
            let mut project = self.project.lock();
            let project = project.as_mut()?;
            let pages = project.session.lock().project();
            let moved = project.page.is_none_or(|page| pages.page(page).is_err());
            if moved {
                project.page = pages.pages().next().map(|page| page.id);
            }
            (Arc::clone(&project.session), project.page, moved)
        };
        if moved {
            self.emit(events::PAGE, page);
        }
        Some((session, page))
    }

    /// Shows `page` of `session` on the canvas, or an empty canvas when the
    /// project has no page.
    fn show(&self, (session, page): (Arc<Mutex<Session>>, Option<PageId>)) -> Result<()> {
        match page {
            Some(page) => self.screen.show(self.render(&session, page)?),
            None => self
                .screen
                .set_canvas(Some(Canvas::new(session, self.renderer.clone()))),
        }
        Ok(())
    }

    /// `page` as `session` holds it now. The snapshot reads the project the
    /// frame drew, under the same lock, so the canvas never takes it.
    fn render(&self, session: &Mutex<Session>, page: PageId) -> Result<Snapshot> {
        let session = session.lock();
        let frame = self.renderer.render(&session, page)?;
        Snapshot::new(frame, &session).context("the page is gone")
    }

    /// Carries out what a canvas gesture did, and gives back the page as its
    /// edits left it, so the redraw that ends the gesture shows them: the
    /// relay renders a change only later, on another thread, and the canvas
    /// would meanwhile draw the page from before the edit.
    pub fn handle(self: &Arc<Self>, effects: Vec<Effect>, record_selection: bool) -> Option<Snapshot> {
        let mut edited = false;
        let mut created_layers = Vec::new();
        for effect in effects {
            let result = match effect {
                Effect::Edit(edit) => self.edit(edit).map(|change| {
                    edited = true;
                    created_layers.extend(change.created.into_iter().filter_map(|created| match created {
                        Created::Layer { id, .. } => Some(id),
                        _ => None,
                    }));
                }),
                Effect::Selection(layers) => {
                    tracing::info!(layers = layers.len(), "selection");
                    if let Some(open) = self.project.lock().as_ref()
                        && let Some(page) = open.page
                    {
                        let automatic = layers.iter().any(|id| created_layers.contains(id));
                        open.session.lock().select_layers(
                            page,
                            layers.clone(),
                            record_selection && !automatic,
                        );
                    }
                    self.emit(events::SELECTION, layers);
                    Ok(())
                }
                // The screen tells the camera itself.
                Effect::Camera(_) => Ok(()),
                Effect::Color(color) => {
                    tracing::info!(?color, "color picked");
                    self.emit(events::COLOR, color);
                    Ok(())
                }
                Effect::BrushDiameter(diameter) => {
                    self.emit(events::BRUSH_DIAMETER, diameter);
                    Ok(())
                }
                Effect::Notice(notice) => {
                    tracing::info!(?notice, "canvas notice");
                    self.emit(events::NOTICE, notice);
                    Ok(())
                }
                Effect::Copy(text) => arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.set_text(text))
                    .context("the clipboard took no text"),
                // Windows' clipboard ends lines with CR LF; text breaks
                // lines with LF.
                Effect::Paste => arboard::Clipboard::new()
                    .and_then(|mut clipboard| clipboard.get_text())
                    .map(|text| self.screen.insert(text.replace("\r\n", "\n")))
                    .context("the clipboard has no text"),
                Effect::Inpaint {
                    page,
                    rect,
                    mask,
                    raster,
                } => self
                    .process(
                        Target::Area {
                            page,
                            area: Area { rect, pixels: mask },
                            prompt: None,
                            raster,
                        },
                        BTreeSet::from([Stage::Inpaint]),
                        None,
                    )
                    .map(drop),
            };
            if let Err(error) = result {
                tracing::error!("failed to carry out a canvas gesture: {error:#}");
            }
        }
        if !edited {
            return None;
        }
        let (session, page) = {
            let project = self.project.lock();
            let project = project.as_ref()?;
            (Arc::clone(&project.session), project.page?)
        };
        self.render(&session, page)
            .inspect_err(|error| tracing::error!("failed to show a canvas edit: {error:#}"))
            .ok()
    }
}
