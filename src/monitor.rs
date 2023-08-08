mod branch;
mod hotness;

use std::path::{Path, PathBuf};

use walrus::Module;

pub enum Monitor {
    Branch,
    Hotness,
}

impl Monitor {
    fn name(&self) -> &str {
        match self {
            Monitor::Branch => "branches",
            Monitor::Hotness => "hotness",
        }
    }
}

const MEMREGION: &str = "instrument";
const MEMUNIT: usize = 64;
const COUNTSIZE: usize = 4; // Size in bytes for storing a count

/// Adds monitor instrumentation bytecode to an existing
/// WASM module.
pub fn add_monitor(module: Module, monitor: Monitor, path: &Path) -> walrus::Result<()> {
    let instrumented_module = match monitor {
        Monitor::Branch => branch::instrument(module),
        Monitor::Hotness => hotness::instrument(module),
    };

    write_module(instrumented_module, &monitor.name(), path)
}

/// Writes the WASM module to the given path adding
/// monitor name to the file name.
fn write_module(mut module: Module, monitor_name: &str, path: &Path) -> walrus::Result<()> {
    let file_stem = path.file_stem().unwrap().to_str().unwrap();
    let extension = path.extension().unwrap().to_str().unwrap();
    let new_file_stem = format!("{}-{}", file_stem, monitor_name);
    let new_file_name = PathBuf::from(format!("{}.{}", new_file_stem, extension));

    module.emit_wasm_file(path.with_file_name(new_file_name))
}
