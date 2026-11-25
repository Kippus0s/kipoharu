//! The native packages a device needs, each pinned to a version.

mod archive;
#[cfg(any(target_os = "linux", target_os = "windows"))]
mod cuda;
mod ggml;
mod host;
mod release;
#[cfg(all(
    target_arch = "x86_64",
    any(target_os = "linux", target_os = "windows")
))]
mod rocm;
mod torch;

use std::{
    mem::forget,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use futures::future::BoxFuture;

use self::{
    archive::{LIBRARIES, extract},
    host::Host,
    release::Release,
};
use crate::{Backend, Device, Runtime, store};

/// A native package a GPU needs. Its folder in the store follows from the host
/// and the GPU, so `load` finds what `download` put there.
trait Package: Send + Sync {
    /// Downloads it into the store.
    fn download(&self, host: Host, backend: Backend) -> BoxFuture<'_, Result<()>>;

    /// Loads it from the store for the life of the process, and updates `gpu`
    /// with what that shows.
    fn load(&self, host: Host, gpu: &mut Device) -> Result<()>;

    /// The version folders it uses in the store. A start removes the other
    /// versions beside them.
    fn versions(&self) -> Vec<PathBuf>;
}

/// Shared GPU dependencies, initialized once before any model backend.
fn drivers(backend: Backend) -> Vec<Box<dyn Package>> {
    let mut packages: Vec<Box<dyn Package>> = Vec::new();
    match backend {
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        Backend::Cuda { .. } => packages.push(Box::new(cuda::Cuda)),
        #[cfg(all(
            target_arch = "x86_64",
            any(target_os = "linux", target_os = "windows")
        ))]
        Backend::Rocm { target } => packages.push(Box::new(rocm::Rocm { target })),
        _ => {}
    }
    packages
}

/// Downloads what `gpu` needs and loads it. Everything downloads before anything
/// loads, so a transfer that fails leaves nothing loaded. Old versions go in
/// the background, so models don't wait for them. Returns `gpu` as its packages
/// see it.
pub(crate) async fn start_driver(gpu: &Device) -> Result<Device> {
    start(gpu, drivers(gpu.backend)).await
}

pub(crate) async fn start_runtime(gpu: &Device, runtime: Runtime) -> Result<Device> {
    let package: Box<dyn Package> = match runtime {
        Runtime::Torch => Box::new(torch::Torch),
        Runtime::Llama => Box::new(ggml::Llama),
        Runtime::Diffusion => Box::new(ggml::Diffusion),
    };
    start(gpu, vec![package]).await
}

async fn start(gpu: &Device, packages: Vec<Box<dyn Package>>) -> Result<Device> {
    let host = Host::current()?;
    for package in &packages {
        package.download(host, gpu.backend).await?;
    }
    let mut gpu = gpu.clone();
    for package in &packages {
        package.load(host, &mut gpu)?;
    }
    tokio::spawn(store::remove_others(
        packages
            .iter()
            .flat_map(|package| package.versions())
            .collect(),
    ));
    Ok(gpu)
}

/// Loads a library that bound ones depend on (CUDA's, ROCm's, and libtorch's
/// own) for the life of the process; a bound library's own load finds it
/// already loaded.
fn preload(path: &Path, global: bool) -> Result<()> {
    #[cfg(windows)]
    let library: libloading::Library = {
        let _ = global;
        use libloading::os::windows::{
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, Library,
        };
        // The wheels' paths use forward slashes, but the loader finds the
        // library's folder, where its dependencies are, only in a path with
        // backslashes; rebuilt from its components, the path has them.
        let path: std::path::PathBuf = path.components().collect();
        unsafe {
            Library::load_with_flags(
                &path,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        }
        .with_context(|| format!("couldn't load {}", path.display()))?
        .into()
    };
    #[cfg(unix)]
    let library: libloading::Library = {
        use libloading::os::unix::{Library, RTLD_GLOBAL, RTLD_LOCAL, RTLD_NOW};
        let visibility = if global { RTLD_GLOBAL } else { RTLD_LOCAL };
        unsafe { Library::open(Some(path), RTLD_NOW | visibility) }
            .with_context(|| format!("couldn't load {}", path.display()))?
            .into()
    };
    forget(library);
    Ok(())
}
