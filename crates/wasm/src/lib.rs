use miette::{ErrReport, GraphicalReportHandler, GraphicalTheme, ThemeCharacters, ThemeStyles};
use semver::Version;
use std::collections::HashMap;
use veryl_analyzer::Analyzer;
use veryl_emitter::Emitter;
use veryl_formatter::Formatter;
use veryl_metadata::{Build, Format, Metadata, Project};
use veryl_parser::Parser;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    pub fn alert(s: &str);
}

#[wasm_bindgen]
pub struct ParseResult {
    code: String,
    err: String,
}

#[wasm_bindgen]
impl ParseResult {
    #[wasm_bindgen]
    pub fn code(&self) -> String {
        self.code.clone()
    }

    #[wasm_bindgen]
    pub fn err(&self) -> String {
        self.err.clone()
    }
}

fn render_err(err: ErrReport) -> String {
    let mut out = String::new();
    GraphicalReportHandler::new_themed(GraphicalTheme {
        characters: ThemeCharacters::emoji(),
        styles: ThemeStyles::none(),
    })
    .with_width(80)
    .render_report(&mut out, err.as_ref())
    .unwrap();
    out
}

fn metadata() -> Metadata {
    Metadata {
        project: Project {
            name: "".into(),
            version: Version::parse("0.0.0").unwrap(),
            authors: vec![],
            description: None,
            license: None,
            repository: None,
        },
        build: Build::default(),
        format: Format::default(),
        dependencies: HashMap::new(),
        metadata_path: "".into(),
    }
}

#[wasm_bindgen]
pub fn parse(source: &str) -> ParseResult {
    let metadata = metadata();
    match Parser::parse(source, &"") {
        Ok(parser) => {
            let analyzer = Analyzer::new::<&str>(&[]);
            let _ = analyzer.analyze_pass1(source, &"", &parser.veryl);
            let _ = analyzer.analyze_pass2(source, &"", &parser.veryl);
            let _ = analyzer.analyze_pass3(source, &"", &parser.veryl);

            let mut emitter = Emitter::new(&metadata);
            emitter.emit(&parser.veryl);
            ParseResult {
                code: emitter.as_str().to_owned(),
                err: "".to_owned(),
            }
        }
        Err(e) => ParseResult {
            code: "".to_owned(),
            err: render_err(e.into()),
        },
    }
}

#[wasm_bindgen]
pub fn format(source: &str) -> ParseResult {
    let metadata = metadata();
    match Parser::parse(source, &"") {
        Ok(parser) => {
            let mut formatter = Formatter::new(&metadata);
            formatter.format(&parser.veryl);
            ParseResult {
                code: formatter.as_str().to_owned(),
                err: "".to_owned(),
            }
        }
        Err(e) => ParseResult {
            code: "".to_owned(),
            err: render_err(e.into()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn get_default_code() -> String {
        let path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let mut path = PathBuf::from(path);
        path.push("playground");
        path.push("index.html");
        let text = std::fs::read_to_string(path).unwrap();
        let mut code = false;
        let mut code_text = String::new();
        for line in text.lines() {
            if line.contains("</textarea") {
                code = false;
            }
            if code {
                code_text.push_str(&format!("{line}\n"));
            }
            if line.contains("<textarea") {
                code = true;
            }
        }
        code_text
    }

    #[test]
    fn build_default_code() {
        let text = get_default_code();
        let ret = parse(&text);

        assert_eq!(ret.err, "");
        assert_ne!(ret.code, "");
    }

    #[test]
    fn format_default_code() {
        let text = get_default_code();
        let ret = format(&text);

        assert_eq!(ret.err, "");
        assert_eq!(ret.code, text);
    }
}
