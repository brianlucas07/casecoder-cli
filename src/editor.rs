use rustyline::completion::Completer;
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Editor, Helper};
use std::borrow::Cow;

pub struct MeuHighlighter;

impl Highlighter for MeuHighlighter {
    fn highlight<'l>(
        &self,
        line: &'l str,
        _pos: usize,
    ) -> Cow<'l, str> {
        Cow::Owned(format!("\x1b[34m{}\x1b[0m", line))
    }

    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        let _ = default;
        Cow::Owned(format!("\x1b[34m{}\x1b[0m", prompt))
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
        // Retornar true é essencial para o Rustyline chamar highlight() enquanto o usuário digita
        true
    }
}

impl Completer for MeuHighlighter {
    type Candidate = String;
}

impl Hinter for MeuHighlighter {
    type Hint = String;
}

impl Validator for MeuHighlighter {}

impl Helper for MeuHighlighter {}

pub type CliEditor = Editor<MeuHighlighter, rustyline::history::DefaultHistory>;

pub fn criar_editor() -> Result<CliEditor, rustyline::error::ReadlineError> {
    let config = rustyline::Config::builder().build();
    let mut rl = Editor::with_config(config)?;
    rl.set_helper(Some(MeuHighlighter));
    Ok(rl)
}
