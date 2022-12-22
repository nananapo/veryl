use dashmap::DashMap;
use ropey::Rope;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer};
use veryl_analyzer::Analyzer;
use veryl_formatter::Formatter;
use veryl_parser::{miette, Parser, ParserError};

#[derive(Debug)]
pub struct Backend {
    client: Client,
    document_map: DashMap<String, Rope>,
    parser_map: DashMap<String, Parser>,
}

struct TextDocumentItem {
    uri: Url,
    text: String,
    version: i32,
}

impl Backend {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            document_map: DashMap::new(),
            parser_map: DashMap::new(),
        }
    }

    async fn on_change(&self, params: TextDocumentItem) {
        let path = params.uri.to_string();
        let rope = Rope::from_str(&params.text);
        let text = rope.to_string();

        let diag = match Parser::parse(&text, &path) {
            Ok(x) => {
                let mut analyzer = Analyzer::new(&text);
                analyzer.analyze(&x.veryl);
                let ret: Vec<_> = analyzer
                    .errors
                    .drain(0..)
                    .map(|x| {
                        let x: miette::ErrReport = x.into();
                        Backend::to_diag(x, &rope)
                    })
                    .collect();
                self.parser_map.insert(path.clone(), x);
                ret
            }
            Err(x) => {
                self.parser_map.remove(&path);
                vec![Backend::to_diag(x, &rope)]
            }
        };
        self.client
            .publish_diagnostics(params.uri, diag, Some(params.version))
            .await;

        self.document_map.insert(path, rope);
    }

    fn to_diag(err: miette::ErrReport, rope: &Rope) -> Diagnostic {
        let miette_diag: &dyn miette::Diagnostic = err.as_ref();

        let range = if let Some(mut labels) = miette_diag.labels() {
            labels.next().map_or(Range::default(), |label| {
                let line = rope.byte_to_line(label.offset());
                let pos = label.offset() - rope.line_to_byte(line);
                let line = line as u32;
                let pos = pos as u32;
                let len = label.len() as u32;
                Range::new(Position::new(line, pos), Position::new(line, pos + len))
            })
        } else {
            Range::default()
        };

        let code = miette_diag
            .code()
            .map(|d| NumberOrString::String(format!("{d}")));

        let message = if let Some(x) = err.downcast_ref::<ParserError>() {
            match x {
                ParserError::PredictionErrorWithExpectations {
                    unexpected_tokens, ..
                } => {
                    format!(
                        "Syntax Error: {}",
                        Backend::demangle_unexpected_token(&unexpected_tokens[0].to_string())
                    )
                }
                _ => format!("Syntax Error: {}", x),
            }
        } else {
            format!("Semantic Error: {}", err)
        };

        Diagnostic::new(
            range,
            Some(DiagnosticSeverity::ERROR),
            code,
            Some(String::from("veryl-ls")),
            message,
            None,
            None,
        )
    }

    fn demangle_unexpected_token(text: &str) -> String {
        if text.contains("LBracketAMinusZ") {
            String::from("Unexpected token: Identifier")
        } else if text.contains("LBracket0Minus") {
            String::from("Unexpected token: Number")
        } else {
            text.replace("LA(1) (", "").replace(')', "")
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                workspace: Some(WorkspaceServerCapabilities {
                    workspace_folders: Some(WorkspaceFoldersServerCapabilities {
                        supported: Some(true),
                        change_notifications: Some(OneOf::Left(true)),
                    }),
                    file_operations: None,
                }),
                document_formatting_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            server_info: Some(ServerInfo {
                name: String::from("veryl-ls"),
                version: Some(String::from(env!("CARGO_PKG_VERSION"))),
            }),
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        self.client.log_message(MessageType::INFO, "did_open").await;

        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: params.text_document.text,
            version: params.text_document.version,
        })
        .await
    }

    async fn did_change(&self, mut params: DidChangeTextDocumentParams) {
        self.client
            .log_message(MessageType::INFO, "did_change")
            .await;

        self.on_change(TextDocumentItem {
            uri: params.text_document.uri,
            text: std::mem::take(&mut params.content_changes[0].text),
            version: params.text_document.version,
        })
        .await
    }

    async fn formatting(&self, params: DocumentFormattingParams) -> Result<Option<Vec<TextEdit>>> {
        let path = params.text_document.uri.to_string();
        if let Some(rope) = self.document_map.get(&path) {
            let line = rope.len_lines() as u32;
            if let Some(parser) = self.parser_map.get(&path) {
                let mut formatter = Formatter::new();
                formatter.format(&parser.veryl);

                let text_edit = TextEdit {
                    range: Range::new(Position::new(0, 0), Position::new(line, u32::MAX)),
                    new_text: formatter.as_str().to_string(),
                };

                return Ok(Some(vec![text_edit]));
            }
        }
        Ok(None)
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }
}