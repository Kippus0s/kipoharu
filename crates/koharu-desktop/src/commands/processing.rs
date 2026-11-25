//! The pipeline's runs on the open project.

use std::{collections::BTreeSet, sync::Arc};

use koharu_pipeline::{Area, Event, Inpainter, PipelineConfig, RunId, Stage, Target};
use tauri::State;

use super::Result;
use crate::app::App;

/// Runs `stages` on `target`; each run's events go out as `pipeline`.
#[tauri::command]
#[specta::specta]
pub fn run_pipeline(app: State<'_, Arc<App>>, target: Target, stages: Vec<Stage>) -> Result<RunId> {
    Ok(app.process(target, stages.into_iter().collect(), None)?)
}

/// Fills the area the marquee and the lasso marked with `model` after
/// `prompt`, or the inpainter's own prompt when there's none, as Photoshop's
/// Generative Fill does with the model its bar shows; `None` while no area is
/// marked. The model is this run's only, in the settings' place.
#[tauri::command]
#[specta::specta]
pub async fn fill(
    app: State<'_, Arc<App>>,
    prompt: Option<String>,
    model: Inpainter,
) -> Result<Option<RunId>> {
    tracing::info!(prompted = prompt.is_some(), %model, "fill");
    let Some((page, rect, pixels)) = app.screen.area().await.ok().flatten() else {
        return Ok(None);
    };
    let mut config = koharu_config::get::<PipelineConfig>()?;
    config.inpainter.model = model;
    let target = Target::Area {
        page,
        area: Area { rect, pixels },
        prompt,
        raster: None,
    };
    Ok(Some(app.process(
        target,
        BTreeSet::from([Stage::Inpaint]),
        Some(config),
    )?))
}

/// Stops every run, running and queued; what they finished stays.
#[tauri::command]
#[specta::specta]
pub fn stop_pipeline(app: State<'_, Arc<App>>) {
    app.pipeline.stop();
}

/// Stops the pipeline and waits until its models are released. Each loads
/// again only when a later stage needs it.
#[tauri::command]
#[specta::specta]
pub async fn unload(app: State<'_, Arc<App>>) -> Result<()> {
    tracing::info!("unload");
    let (agent_run, conversation) = {
        let mut project = app.project.lock();
        match project.as_mut() {
            Some(project) => (project.run.take(), project.conversation.take()),
            None => (None, None),
        }
    };
    if let Some((_, task)) = agent_run {
        task.abort();
        let _ = task.await;
    }
    // A final local model handle can join its native worker; do not block
    // the async runtime while destroying the agent conversation.
    tokio::task::spawn_blocking(move || drop(conversation)).await?;
    app.pipeline.unload().await?;
    tracing::info!("pipeline models unloaded");
    Ok(())
}

/// What every run told so far, for an interface that loaded late.
#[tauri::command]
#[specta::specta]
pub fn get_pipeline_events(app: State<'_, Arc<App>>) -> Vec<Event> {
    app.pipeline.told()
}
