use std::io::{self, Write};
use tempra_repl::repl::{ReplOutcome, ReplSession};

pub fn repl_command() -> Result<(), String> {
    println!("Tempra REPL (Bootstrap)");
    println!("Type :help for commands, :quit to exit.");

    let mut session = ReplSession::new();
    let mut buffer = String::new();
    let mut is_multiline = false;

    loop {
        if is_multiline {
            print!("... ");
        } else {
            print!("> ");
        }
        io::stdout().flush().map_err(|e| e.to_string())?;

        let mut line = String::new();
        if io::stdin().read_line(&mut line).map_err(|e| e.to_string())? == 0 {
            break; // EOF
        }

        buffer.push_str(&line);

        // Simple balanced brace check
        if is_incomplete(&buffer) {
            is_multiline = true;
            continue;
        }

        match session.eval_input(&buffer) {
            ReplOutcome::Value(val) => {
                // Don't print Unit
                if !matches!(val, tempra_interpreter::value::Value::Unit) {
                    println!("{}", val);
                }
            }
            ReplOutcome::Empty => {}
            ReplOutcome::Quit => break,
            ReplOutcome::Diagnostic(msg) => {
                println!("{}", msg);
            }
        }

        buffer.clear();
        is_multiline = false;
    }

    Ok(())
}

fn is_incomplete(input: &str) -> bool {
    let mut brace_depth = 0;
    let mut paren_depth = 0;
    let mut bracket_depth = 0;

    for c in input.chars() {
        match c {
            '{' => brace_depth += 1,
            '}' => brace_depth -= 1,
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            '[' => bracket_depth += 1,
            ']' => bracket_depth -= 1,
            _ => {}
        }
    }

    brace_depth > 0 || paren_depth > 0 || bracket_depth > 0
}
