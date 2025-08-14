use anyhow::{Context, Result};
use std::{
    collections::HashMap,
    fs,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tower_lsp::lsp_types::*;
use tower_lsp::{LanguageServer, LspService, Server};

#[derive(Default)]
struct Docs {
    map: HashMap<String, String>,
}
impl Docs {
    fn load(path: PathBuf) -> Result<Self> {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("reading {}", path.display()))?;
        let map: HashMap<String, String> =
            toml::from_str(&text).context("parsing alloy-hover.toml")?;
        Ok(Self { map })
    }
    fn get(&self, key: &str) -> Option<String> {
        self.map.get(key).cloned()
    }
}

struct Backend {
    files: Arc<RwLock<HashMap<Url, String>>>,
    docs: Docs,
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(
        &self,
        _params: InitializeParams,
    ) -> tower_lsp::jsonrpc::Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "alloy-hover-lsp".into(),
                version: Some("0.1.0".into()),
            }),
        })
    }

    async fn initialized(&self, _params: InitializedParams) {}

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.files
            .write()
            .unwrap()
            .insert(params.text_document.uri, params.text_document.text);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.into_iter().last() {
            self.files
                .write()
                .unwrap()
                .insert(params.text_document.uri, change.text);
        }
    }

    async fn hover(
        &self,
        params: HoverParams,
    ) -> tower_lsp::jsonrpc::Result<Option<Hover>> {
        let uri = params.text_document_position_params.text_document.uri;
        let pos = params.text_document_position_params.position;

        let text = {
            let guard = self.files.read().unwrap();
            guard.get(&uri).cloned()
        };
        let Some(text) = text else { return Ok(None) };

        let line = text.lines().nth(pos.line as usize).unwrap_or_default();

        let mut start = pos.character as usize;
        let mut end = start;
        let is_word = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '.';

        while start > 0 && line.chars().nth(start - 1).map(is_word).unwrap_or(false) {
            start -= 1;
        }
        while end < line.len() && line.chars().nth(end).map(is_word).unwrap_or(false) {
            end += 1;
        }

        let word = line.get(start..end).unwrap_or("").trim_matches('"');
        if word.is_empty() {
            return Ok(None);
        }

        if let Some(md) = self.docs.get(word) {
            return Ok(Some(Hover {
                contents: HoverContents::Markup(MarkupContent {
                    kind: MarkupKind::Markdown,
                    value: md,
                }),
                range: Some(Range {
                    start: Position {
                        line: pos.line,
                        character: start as u32,
                    },
                    end: Position {
                        line: pos.line,
                        character: end as u32,
                    },
                }),
            }));
        }

        Ok(None)
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let docs_path =
        std::env::var("ALLOY_HOVER_DOCS").unwrap_or_else(|_| "docs/alloy-hover.toml".into());
    let docs = Docs::load(PathBuf::from(docs_path))?;
    let files = Arc::new(RwLock::new(HashMap::new()));

    // Requires tokio feature: io-std
    let (stdin, stdout) = (tokio::io::stdin(), tokio::io::stdout());

    let (service, socket) = LspService::new(|_client| Backend { files: files.clone(), docs });
    Server::new(stdin, stdout, socket).serve(service).await;
    Ok(())
}
