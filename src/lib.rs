use std::{
    env::current_dir,
    fs,
    fs::create_dir_all,
    path::{Path, PathBuf},
};

use zed_extension_api::{
    self as zed, Extension, LanguageServerId, Worktree, Os, current_platform,
    serde_json::{self, Value},
    settings::LspSettings,
    register_extension,
};

const PATH_TO_STR_ERROR: &str = "failed to convert path to string";

struct ConfAlloy {
    cached_binary_path: Option<PathBuf>,
    cached_docs_path:   Option<PathBuf>,
}

impl ConfAlloy {
    const HOVER_SERVER_ID: &'static str = "alloy-hover";
    const BINARY_NAME:     &'static str = "alloy-hover-lsp";
    // Ship this file in your extension repo at: ./docs/alloy-hover.toml
    const DOCS_TOML:       &'static str = include_str!("../docs/alloy-hover.toml");

    fn language_server_binary_path(
        &mut self,
        _language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> zed::Result<PathBuf> {
        // Cache hit?
        if let Some(p) = &self.cached_binary_path {
            if fs::metadata(p).is_ok_and(|m| m.is_file()) {
                return Ok(p.clone());
            }
        }

        // Prefer PATH (like Java does)
        if let Some(path) = worktree.which(Self::BINARY_NAME) {
            let p = PathBuf::from(path);
            self.cached_binary_path = Some(p.clone());
            return Ok(p);
        }

        Err(format!(
            "Could not find `{}` on PATH",
            Self::BINARY_NAME
        ))
    }

    /// Ensure the docs file exists in our extension's work directory,
    /// then return its *absolute* path. Mirrors how the Java extension
    /// manages its own downloaded/cached assets.
    fn docs_file_path(&mut self) -> zed::Result<PathBuf> {
        if let Some(p) = &self.cached_docs_path {
            if fs::metadata(p).is_ok_and(|m| m.is_file()) {
                return Ok(p.clone());
            }
        }

        // The extension runs with a stable CWD. Use a subdir we control.
        // (This matches the "jdtls"/"lombok" dirs in the Java example.)
        let mut base = current_dir().map_err(|e| format!("could not get current dir: {e}"))?;

        // Zed on Windows presents a leading "/" sometimes; Java trims it.
        if current_platform().0 == Os::Windows {
            if let Ok(stripped) = base.strip_prefix("/") {
                base = stripped.to_path_buf();
            }
        }

        let dir = base.join("alloy-hover");
        create_dir_all(&dir).map_err(|e| format!("failed to create dir `{}`: {e}", dir.display()))?;

        let docs_path = dir.join("alloy-hover.toml");

        // If missing, write the copy we ship in the extension bundle
        if !fs::metadata(&docs_path).is_ok_and(|m| m.is_file()) {
            fs::write(&docs_path, Self::DOCS_TOML)
                .map_err(|e| format!("failed to write `{}`: {e}", docs_path.display()))?;
        }

        self.cached_docs_path = Some(docs_path.clone());
        Ok(docs_path)
    }
}

impl Extension for ConfAlloy {
    fn new() -> Self where Self: Sized {
        Self {
            cached_binary_path: None,
            cached_docs_path: None,
        }
    }

    fn language_server_command(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> zed::Result<zed::Command> {
        if language_server_id.as_ref() != Self::HOVER_SERVER_ID {
            return Err(format!("unknown language server: {}", language_server_id.as_ref()));
        }

        let command = self
            .language_server_binary_path(language_server_id, worktree)?
            .to_str()
            .ok_or(PATH_TO_STR_ERROR)?
            .to_string();

        // Produce an absolute path for the docs we manage
        let mut cwd = current_dir().map_err(|e| format!("could not get current dir: {e}"))?;
        if current_platform().0 == Os::Windows {
            if let Ok(stripped) = cwd.strip_prefix("/") {
                cwd = stripped.to_path_buf();
            }
        }
        let docs_abs = cwd
            .join(self.docs_file_path()?)
            .canonicalize()
            .unwrap_or_else(|_| cwd.join(self.docs_file_path().unwrap()));

        let docs_abs = docs_abs
            .to_str()
            .ok_or(PATH_TO_STR_ERROR)?
            .to_string();

        Ok(zed::Command {
            command,
            args: vec![],           // your LSP doesn't need args
            env: vec![
                ("ALLOY_HOVER_DOCS".to_string(), docs_abs),
            ],
        })
    }

    fn language_server_initialization_options(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> zed::Result<Option<Value>> {
        // Preserve compatibility with Settings UI, same as Java:
        zed::settings::LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .map(|lsp| lsp.initialization_options)
    }

    fn language_server_workspace_configuration(
        &mut self,
        language_server_id: &LanguageServerId,
        worktree: &Worktree,
    ) -> zed::Result<Option<Value>> {
        // Same pattern as Java extension does:
        let mut settings = zed::settings::LspSettings::for_worktree(language_server_id.as_ref(), worktree)
            .map(|lsp| lsp.settings);

        if !matches!(settings, Ok(Some(_))) {
            settings = self
                .language_server_initialization_options(language_server_id, worktree)
                .map(|init_opts| init_opts.and_then(|v| v.get("settings").cloned()));
        }

        settings
    }
}

register_extension!(ConfAlloy);
