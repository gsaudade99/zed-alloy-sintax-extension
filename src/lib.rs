use std::env;
use zed_extension_api::{self as zed, LanguageServerId, Result, serde_json::Value};

struct ConfAlloyExtension;

impl ConfAlloyExtension {
    const HOVER_SERVER_ID: &'static str = "alloy-hover";
    const BINARY_NAME: &'static str = "alloy-hover-lsp";      // name if installed on PATH
    const RELATIVE_BIN: &'static str = "bin/alloy-hover-lsp"; // shipped with the extension
}

impl zed::Extension for ConfAlloyExtension {
    fn new() -> Self {
        Self
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &zed::Worktree,
    ) -> Result<zed::Command> {
        if language_server_id.as_ref() != Self::HOVER_SERVER_ID {
            return Err(format!("unknown language server: {}", language_server_id.as_ref()));
        }

        // 1) Prefer a binary on PATH
        let command_path = if let Some(path) = worktree.which(Self::BINARY_NAME) {
            path
        } else {
            // 2) Fallback to ./bin/alloy-hover-lsp shipped in the extension directory
            sanitize_windows_path(env::current_dir().unwrap())
                .join(Self::RELATIVE_BIN)
                .to_string_lossy()
                .to_string()
        };

        Ok(zed::Command {
            command: command_path,
            args: vec![], // our tiny LSP doesn't need args
            env: vec![("ALLOY_HOVER_DOCS".into(), "docs/alloy-hover.toml".into())],
        })
    }

    // (Optional) If you want to pass init options or workspace config later:
    fn language_server_initialization_options(
        &mut self,
        _server_id: &LanguageServerId,
        _worktree: &zed::Worktree,
    ) -> Result<Option<Value>> {
        Ok(None)
    }
}

zed::register_extension!(ConfAlloyExtension);

// Same helper used by Zed's own extensions to fix Windows leading "/"
fn sanitize_windows_path(path: std::path::PathBuf) -> std::path::PathBuf {
    use zed_extension_api::{current_platform, Os};
    let (os, _arch) = current_platform();
    match os {
        Os::Mac | Os::Linux => path,
        Os::Windows => path
            .to_string_lossy()
            .to_string()
            .trim_start_matches('/')
            .into(),
    }
}
