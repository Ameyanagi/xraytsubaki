use std::fs;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use xraytsubaki::prelude::{run_feff, FeffExecutionMode, FeffRunRequest};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let backend = args.next().unwrap_or_else(|| "refeff".to_string());
    let input = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: feff_backend_probe <refeff|feffrs> <feff.inp> [iterations] [--all-outputs]")?
        .canonicalize()?;
    let iterations = args
        .next()
        .filter(|value| !value.starts_with("--"))
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(1);
    let keep_all_outputs = std::env::args().any(|arg| arg == "--all-outputs");
    let mode = match backend.as_str() {
        "refeff" => FeffExecutionMode::RefeffPipeline,
        "feffrs" | "feff10" => FeffExecutionMode::Feff10Pipeline,
        _ => return Err(format!("unknown backend '{backend}'").into()),
    };

    println!("iteration,backend,elapsed_ms,path_files");
    for iteration in 1..=iterations {
        let unique = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        let workspace = std::env::temp_dir().join(format!(
            "xraytsubaki-feff-probe-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir_all(&workspace)?;
        let request = FeffRunRequest {
            executable_path: PathBuf::new(),
            workspace_dir: workspace.clone(),
            feffinp: Some(input.clone()),
            mode,
            timeout_sec: Some(600),
            use_sfconv: false,
            keep_all_outputs,
        };

        let started = Instant::now();
        let result = run_feff(&request);
        let elapsed = started.elapsed();
        let cleanup = fs::remove_dir_all(&workspace);
        let result = result?;
        cleanup?;
        println!(
            "{iteration},{backend},{:.3},{}",
            elapsed.as_secs_f64() * 1_000.0,
            result.path_files.len()
        );
    }
    Ok(())
}
