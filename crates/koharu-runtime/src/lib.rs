//! Gets Koharu ready to run models on this machine's GPU.

mod device;
mod hardware;
mod huggingface;
mod packages;
mod store;
mod transfer;

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::Context;
use tokio::sync::OnceCell;

pub use device::{Backend, Device, Memory};
pub use huggingface::Repository;
pub use transfer::{State, Transfer, transfers};

/// Koharu's root: the folder setup put it in, in Windows release builds, where
/// this user can write it, which a per-machine install's, under Program Files,
/// isn't. The store goes in it, and the app's projects beside the store.
pub fn root() -> Option<&'static Path> {
    static ROOT: LazyLock<Option<PathBuf>> = LazyLock::new(|| {
        if !cfg!(all(windows, not(debug_assertions))) {
            return None;
        }
        let executable = std::env::current_exe().expect("Koharu knows where its executable is");
        let folder = executable.parent().expect("an executable is in a folder");
        match tempfile::tempfile_in(folder) {
            Err(error) if error.kind() == ErrorKind::PermissionDenied => None,
            _ => Some(folder.to_owned()),
        }
    });
    ROOT.as_deref()
}

/// The native inference backend a model actually uses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Runtime {
    Torch,
    Llama,
    Diffusion,
}

static DRIVER: OnceCell<Device> = OnceCell::const_new();
static TORCH: OnceCell<Device> = OnceCell::const_new();
static LLAMA: OnceCell<Device> = OnceCell::const_new();
static DIFFUSION: OnceCell<Device> = OnceCell::const_new();

/// Whether the native backend is ready. Never call its FFI before this.
#[must_use]
pub fn is_loaded(runtime: Runtime) -> bool {
    cell(runtime).initialized()
}

/// The Torch device, without initializing Torch as a side effect.
#[must_use]
pub fn torch_device() -> Option<&'static Device> {
    TORCH.get()
}

fn cell(runtime: Runtime) -> &'static OnceCell<Device> {
    match runtime {
        Runtime::Torch => &TORCH,
        Runtime::Llama => &LLAMA,
        Runtime::Diffusion => &DIFFUSION,
    }
}

/// Select the shared device, then start only the requested backend. The
/// native bindings are process-lifetime; model objects are not.
pub async fn device(runtime: Runtime) -> anyhow::Result<&'static Device> {
    let gpu = DRIVER
        .get_or_try_init(|| async {
            let gpu = hardware::candidates()
                .into_iter()
                .next()
                .context("GPU is not available")?;
            packages::start_driver(&gpu).await
        })
        .await?;
    cell(runtime)
        .get_or_try_init(|| async {
            let device = packages::start_runtime(gpu, runtime)
                .await
                .with_context(|| format!("{} can't be used with {runtime:?}", gpu.description))?;
            tracing::info!(?runtime, device = %device.description, "native runtime ready");
            Ok(device)
        })
        .await
}
