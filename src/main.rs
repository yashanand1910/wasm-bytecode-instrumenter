use std::{env, path::Path};

use anyhow::bail;
use walrus::Module;
use wasm_bytecode_instrumenter::monitor::{add_monitor, Monitor};

fn main() -> walrus::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.len() != 2 {
        bail!("Usage: ./bytecode-rewrite <monitor> <filename>");
    }

    let path = Path::new(&args[1]);
    if !path.exists() {
        bail!("File does not exist");
    }

    let monitor: Monitor = match &args[0][..] {
        "branch" => Monitor::Branch,
        "hotness" => Monitor::Hotness,
        name => bail!("Invalid monitor {}", name),
    };

    let module = match Module::from_file(path) {
        Ok(module) => module,
        _ => bail!("Unable to parse module {:?}", path),
    };

    add_monitor(module, monitor, path)
}
