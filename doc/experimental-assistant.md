# Experimental assistant

Open **Assistant** in the top bar. It is a separate, optional window. Reopening Assistant brings the existing conversation forward. Connect to the installed Codex CLI using its existing account, or choose **Device login**. XrayTsubaki does not read or copy the Codex credential file. The integration uses the [Codex app-server protocol](https://learn.chatgpt.com/docs/app-server), including experimental dynamic tools.

- **Plots** includes rendered spectra with the current state when you send a message.
- **Allow changes** enables processing edits, reference selection, path calculation and fitting. It starts off. Navigation and panel changes are available in review mode.
- **Show app** focuses the analysis window. **Focus plots** hides the file browser and inspector.
- **Stop** interrupts the assistant turn. A calculation already running in the analysis engine can finish independently.

Example: “Fit this metallic copper foil. Inspect the processing, use the Cu reference and fit the first shell.” The assistant is instructed to show Data → Normalize → Background → Transform → Structure → Calculate → Paths → Model → Results. Each spectrum's current processing must be inspected before an assistant fit can run. Plot access is evidence for the model to assess; it is not an automatic scientific quality certification.

The app tools expose exact spectrum paths, parameter names, dataset ids, per-spectrum ranges and path assignments. Processing proposals are validated by the actual pipeline before they are applied. Processing and model edits enter the normal undo history. Existing bounds and expressions are preserved when changing a parameter value.

Presentation controls can select stages, spectra, fit steps and k/R/q views, toggle panels, focus the analysis window, resize it and restore its previous size. Arbitrary desktop-window positioning is not exposed by the current GPUI API.

Only sending a message shares the analysis context and enabled plots through the configured Codex account. Context contains source paths, bounded source comments, effective requested settings, model inputs, fit history, additional analyses and the action journal. Imported comments are treated as data. Results remain distinct from the currently edited model. This is an experimental assistant: verify its chosen phase, path list, ranges and scientific interpretation.

Implementation: `codex_client.rs` owns the subprocess/protocol; `app/shell/assistant.rs` owns the window and guarded tool dispatch; `assistant_actions.rs` applies semantic app actions; `assistant_workflow.md` contains the versioned workflow instructions. No application credential storage or separate model API key is required when an existing Codex login is available.
