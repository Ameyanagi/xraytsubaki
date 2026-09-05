//! Optional Codex app-server transport. Authentication stays inside Codex.
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::mpsc,
};
pub(crate) struct Client {
    child: Child,
    tx: mpsc::Sender<Value>,
    rx: mpsc::Receiver<Result<Value, String>>,
    pub directory: PathBuf,
}
impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
impl Client {
    pub fn start() -> Result<Self, String> {
        let executable = executable().ok_or(
            "Codex CLI not found. Install Codex, or set XTS_CODEX to its executable path.",
        )?;
        let directory = std::env::temp_dir().join(format!(
            "xraytsubaki-assistant-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        ));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(&directory).map_err(|e| e.to_string())?;
        let mut child = Command::new(executable)
            .args([
                "-c",
                "features.shell_tool=false",
                "-c",
                "features.unified_exec=false",
                "-c",
                "features.apps=false",
                "-c",
                "features.multi_agent=false",
                "-c",
                "web_search=\"disabled\"",
                "app-server",
            ])
            .current_dir(&directory)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("Could not start Codex: {e}"))?;
        let mut stdin = child.stdin.take().ok_or("Codex stdin unavailable")?;
        let stdout = child.stdout.take().ok_or("Codex stdout unavailable")?;
        let (tx, outgoing) = mpsc::channel::<Value>();
        let (incoming, rx) = mpsc::channel();
        let errors = incoming.clone();
        std::thread::spawn(move || {
            while let Ok(v) = outgoing.recv() {
                if let Err(e) = writeln!(stdin, "{v}").and_then(|_| stdin.flush()) {
                    let _ = errors.send(Err(format!("Codex connection closed: {e}")));
                    break;
                }
            }
        });
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                match line {
                    Ok(line) => match serde_json::from_str(&line) {
                        Ok(v) => {
                            if incoming.send(Ok(v)).is_err() {
                                return;
                            }
                        }
                        Err(_) => {
                            let _ = incoming
                                .send(Err("Codex returned an invalid protocol message".into()));
                        }
                    },
                    Err(e) => {
                        let _ = incoming.send(Err(e.to_string()));
                        return;
                    }
                }
            }
            let _ = incoming.send(Err("Codex disconnected".into()));
        });
        Ok(Self {
            child,
            tx,
            rx,
            directory,
        })
    }
    pub fn send(&self, v: Value) -> Result<(), String> {
        self.tx.send(v).map_err(|_| "Codex disconnected".into())
    }
    pub fn drain(&self) -> Vec<Result<Value, String>> {
        self.rx.try_iter().take(200).collect()
    }
}
fn executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("XTS_CODEX") {
        return Path::new(&path).is_file().then(|| path.into());
    }
    let mut paths: Vec<_> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).map(|p| p.join("codex")).collect())
        .unwrap_or_default();
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        paths.extend([
            home.join(".bun/bin/codex"),
            home.join(".local/bin/codex"),
            home.join(".npm-global/bin/codex"),
        ]);
    }
    paths.extend([
        PathBuf::from("/opt/homebrew/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
    ]);
    paths.into_iter().find(|p| p.is_file())
}
pub(crate) fn initialize(id: u64) -> Value {
    json!({"id":id,"method":"initialize","params":{"clientInfo":{"name":"xraytsubaki","title":"XrayTsubaki","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true}}})
}
pub(crate) fn dynamic_tools() -> Value {
    json!([
    {"type":"function","name":"xray_set_layout","description":"Change visible panels and the main desktop window. File browser/inspector and current/marked plot scope are presentation only. Window actions: focus_app, maximize_app (resize to display), restore_app, resize_app. Read-only analysis mode allows presentation changes.","inputSchema":{"type":"object","properties":{"file_browser":{"type":"boolean"},"inspector":{"type":"boolean"},"plot_scope":{"type":"string","enum":["current","marked"]},"window_action":{"type":"string","enum":["focus_app","maximize_app","restore_app","resize_app"]},"width":{"type":"number"},"height":{"type":"number"}},"additionalProperties":false}},
    {"type":"function","name":"xray_get_plots","description":"Inspect the current processing stage or fit results: returns fresh plots plus resolved numerical settings. Navigate to Normalize, Background and Transform and inspect each before running a fit. Respects the Plots switch.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}},
    {"type":"function","name":"xray_search_structures","description":"Search the curated reference-structure library and display Structure. Returns candidates with stable ids; do not infer metallic composition merely from an element name.","inputSchema":{"type":"object","properties":{"query":{"type":"string"}},"required":["query"],"additionalProperties":false}},
    {"type":"function","name":"xray_choose_structure","description":"Choose a returned curated structure id; display its crystal and 8 Å cluster preview. Returns absorber, edge and site information for inspection.","inputSchema":{"type":"object","properties":{"id":{"type":"string"}},"required":["id"],"additionalProperties":false}},
    {"type":"function","name":"xray_calculate_paths","description":"Run the selected FEFF/ReFEFF backend using the inspected structure, absorber and cluster. Shows Calculate, waits for completion, then shows Paths. Reuse suitable calculated paths; do not duplicate calculations without reason.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}},
    {"type":"function","name":"xray_select_paths","description":"Assign an explicit list of already calculated FEFF files. For multiple-spectrum fits dataset_id is required. Preserves existing expressions; rebuild_parameters=true explicitly regenerates the parameter template and should only be used for a new model.","inputSchema":{"type":"object","properties":{"files":{"type":"array","items":{"type":"string"}},"dataset_id":{"type":"integer"},"rebuild_parameters":{"type":"boolean"}},"required":["files"],"additionalProperties":false}},
    {"type":"function","name":"xray_navigate","description":"Show a spectrum, processing stage, fitting step or plot in the main window. Navigation is available in review mode. Select a catalog spectrum by exact path; select a fitting preview by dataset_id. Valid plots: k, r, q, k+r, mu, normalized_mu, flat_mu.","inputSchema":{"type":"object","properties":{"stage":{"type":"string","enum":["data","normalize","background","transform","fit","series","publish"]},"spectrum":{"type":"string"},"dataset_id":{"type":"integer"},"fit_step":{"type":"string","enum":["structure","calculate","paths","model","results"]},"plot":{"type":"string"}},"additionalProperties":false}},
    {"type":"function","name":"xray_set_fit_ranges","description":"Edit the fit ranges and display their live spectrum preview. In multiple-spectrum mode dataset_id is mandatory; each spectrum keeps its own ranges. Undoable. fitspace values use the exact enum spelling from state.","inputSchema":{"type":"object","properties":{"dataset_id":{"type":"integer"},"ranges":{"type":"object"}},"required":["ranges"],"additionalProperties":false}},
    {"type":"function","name":"xray_set_fit_parameter","description":"Change an existing fit parameter value and show the model editor. For a local parameter specify dataset_id. For a global parameter omit dataset_id. Preserves bounds, expressions and global/local assignments. Undoable.","inputSchema":{"type":"object","properties":{"dataset_id":{"type":"integer"},"name":{"type":"string"},"value":{"type":"number"}},"required":["name","value"],"additionalProperties":false}},

        {"type":"function","name":"xray_get_state","description":"Read current XrayTsubaki screen, per-spectrum processing settings, fit inputs and recorded results. Returns job state; do not claim completion while processing or fitting is running.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}},
        {"type":"function","name":"xray_set_processing","description":"Update requested normalization, AUTOBK or transform settings on the current spectrum only. The app validates and processes the proposal before applying it. Available only when Allow changes is enabled; undoable.","inputSchema":{"type":"object","properties":{"spectrum":{"type":"string","description":"Exact current_spectrum path from xray_get_state"},"changes":{"type":"object","description":"PipelineParams field names mapped to numeric values, enum names, or null for Auto. Import settings cannot be changed."}},"required":["spectrum","changes"],"additionalProperties":false}},
        {"type":"function","name":"xray_run_fit","description":"Run the currently configured fit, including per-spectrum paths and global/local parameters. Waits for completion. Does not invent or add paths.","inputSchema":{"type":"object","properties":{},"additionalProperties":false}}
       ])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn device_code_protocol_does_not_read_credentials() {
        let init = initialize(1);
        assert_eq!(init["params"]["capabilities"]["experimentalApi"], true);
        assert_eq!(dynamic_tools().as_array().unwrap().len(), 12);
    }
    #[test]
    #[ignore = "requires an installed Codex CLI; reads account status without starting a model turn"]
    fn installed_codex_auth_handshake() {
        let c = Client::start().unwrap();
        c.send(initialize(1)).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        let mut account = false;
        while std::time::Instant::now() < deadline {
            for msg in c.drain() {
                let v = msg.unwrap();
                if v["id"] == 1 {
                    assert!(v.get("error").is_none(), "initialization failed");
                    c.send(json!({"method":"initialized","params":{}})).unwrap();
                    c.send(json!({"id":2,"method":"account/read","params":{"refreshToken":false}}))
                        .unwrap();
                }
                if v["id"] == 2 {
                    assert!(v.get("error").is_none(), "account read failed");
                    account = true;
                }
            }
            if account {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(40));
        }
        assert!(account, "account status timed out");
    }
}
