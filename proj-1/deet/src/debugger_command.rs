pub enum DebuggerCommand {
    Quit,
    Run(Vec<String>),
    Continue,
    Backtrace,
    Break(String),
    Print,
    Next,
}

impl DebuggerCommand {
    pub fn from_tokens(tokens: &Vec<&str>) -> Option<DebuggerCommand> {
        match tokens[0] {
            "q" | "quit" => Some(DebuggerCommand::Quit),
            "r" | "run" => {
                let args = tokens[1..].to_vec();
                Some(DebuggerCommand::Run(
                    args.iter().map(|s| s.to_string()).collect(),
                ))
            },
            // Default case:
            "c" | "cont" | "continue" => Some(DebuggerCommand::Continue),
            "bk" | "back" | "backtrace" => Some(DebuggerCommand::Backtrace),
            "b" | "break" => {
                if tokens.len() == 2 {
                    Some(DebuggerCommand::Break(tokens[1].to_string()))
                } else {
                    eprintln!("Missing the argument of BreakPoint");
                    None
                }
            },
            "p" | "print" => Some(DebuggerCommand::Print),
            "n" | "next" => Some(DebuggerCommand::Next),
            _ => None,
        }
    }
}
