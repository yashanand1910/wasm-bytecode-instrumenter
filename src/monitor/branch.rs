use std::path::Path;

use walrus::Module;

/// Adds branch instrumentation bytecode to a module
pub fn instrument(mut _module: Module, _path: &Path) -> Module {
    _module
}
