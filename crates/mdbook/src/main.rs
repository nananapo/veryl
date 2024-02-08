use clap::{Arg, ArgMatches, Command};
use line_col::LineColLookup;
use mdbook::book::{Book, BookItem};
use mdbook::errors::Error;
use mdbook::preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext};
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag, TagEnd};
use regex::Regex;
use semver::{Version, VersionReq};
use std::io;
use std::process;

pub fn make_app() -> Command {
    Command::new("veryl")
        .about("A mdbook preprocessor which does precisely nothing")
        .subcommand(
            Command::new("supports")
                .arg(Arg::new("renderer").required(true))
                .about("Check whether a renderer is supported by this preprocessor"),
        )
}

fn main() {
    let matches = make_app().get_matches();

    let preprocessor = Veryl;

    if let Some(sub_args) = matches.subcommand_matches("supports") {
        handle_supports(&preprocessor, sub_args);
    } else if let Err(e) = handle_preprocessing(&preprocessor) {
        eprintln!("{e}");
        process::exit(1);
    }
}

fn handle_preprocessing(pre: &dyn Preprocessor) -> Result<(), Error> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    let book_version = Version::parse(&ctx.mdbook_version)?;
    let version_req = VersionReq::parse(mdbook::MDBOOK_VERSION)?;

    if !version_req.matches(&book_version) {
        eprintln!(
            "Warning: The {} plugin was built against version {} of mdbook, \
             but we're being called from version {}",
            pre.name(),
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = pre.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(())
}

fn handle_supports(pre: &dyn Preprocessor, sub_args: &ArgMatches) -> ! {
    let renderer = sub_args
        .get_one::<String>("renderer")
        .expect("Required argument");
    let supported = pre.supports_renderer(renderer);

    // Signal whether the renderer is supported by exiting with 1 or 0.
    if supported {
        process::exit(0);
    } else {
        process::exit(1);
    }
}

/// A veryl preprocessor.
#[derive(Default)]
pub struct Veryl;

impl Preprocessor for Veryl {
    fn name(&self) -> &str {
        "veryl"
    }

    fn run(&self, _ctx: &PreprocessorContext, mut book: Book) -> Result<Book, Error> {
        let re_hiding_code_line = Regex::new("(?m)^# .*\n").unwrap();
        let re_hiding_code_indicator = Regex::new("(?m)^# ").unwrap();
        let mut in_code = false;
        let mut total_success = true;
        book.for_each_mut(|item| {
            if let BookItem::Chapter(chapter) = item {
                let path = chapter
                    .source_path
                    .as_ref()
                    .map(|x| x.to_string_lossy())
                    .unwrap_or_else(|| "".into());
                let lookup = LineColLookup::new(&chapter.content);
                let mut chapter_skip = true;
                let mut chapter_success = true;
                let mut code_blocks = Vec::new();
                for (event, range) in Parser::new(&chapter.content).into_offset_iter() {
                    match event {
                        Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(x))) => {
                            if x.as_ref().starts_with("veryl") {
                                in_code = true;
                            }
                        }
                        Event::End(TagEnd::CodeBlock) => {
                            in_code = false;
                        }
                        Event::Text(x) => {
                            if in_code {
                                let replaced_code = re_hiding_code_line.replace_all(x.as_ref(), "");
                                code_blocks.push((x.to_string(), replaced_code.to_string()));

                                chapter_skip = false;
                                let x = re_hiding_code_indicator.replace_all(x.as_ref(), "");
                                let ret = veryl_parser::Parser::parse(&x, &"");
                                let (line, col) = lookup.get(range.start);
                                if ret.is_err() {
                                    eprintln!("veryl parse failed : {path}:{line}:{col}");
                                    total_success = false;
                                    chapter_success = false;
                                }
                            }
                        }
                        _ => (),
                    }
                }
                if chapter_skip {
                    eprintln!("veryl parse skipped: {path}");
                } else if chapter_success {
                    eprintln!("veryl parse success: {path}");
                }
                for (code_block, replaced_code) in code_blocks {
                    chapter.content = chapter.content.replace(&code_block, &replaced_code);
                }
            }
        });

        if total_success {
            Ok(book)
        } else {
            Err(Error::msg("veryl parse check failed!!!"))
        }
    }

    fn supports_renderer(&self, _renderer: &str) -> bool {
        true
    }
}
