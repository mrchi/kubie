use std::env;
use std::fmt::{self, Display};

use crate::settings::Settings;
use crate::shell::ShellKind;

struct Command {
    content: String,
    shell_kind: ShellKind,
}

impl Command {
    fn new(content: impl Into<String>, shell_kind: ShellKind) -> Command {
        Command {
            content: content.into(),
            shell_kind,
        }
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.shell_kind {
            ShellKind::Fish => write!(f, "({})", self.content),
            _ => write!(f, "$({})", self.content),
        }
    }
}

struct Color<D> {
    color: u32,
    content: D,
    shell_kind: ShellKind,
}

impl<D> Color<D> {
    fn new(color: u32, content: D, shell_kind: ShellKind) -> Color<D> {
        Color {
            color,
            content,
            shell_kind,
        }
    }
}

// FIXME: @Miuler Validate this implementation for nu shell
impl<D> Color<D>
where
    D: Display,
{
    fn isolate<E>(&self, f: &mut fmt::Formatter, content: E) -> fmt::Result
    where
        E: Display,
    {
        match self.shell_kind {
            ShellKind::Fish | ShellKind::Xonsh => write!(f, "{content}"),
            ShellKind::Zsh => write!(f, "%{{{content}%}}"),
            ShellKind::Bash => write!(f, "\\[{content}\\]"),
            _ => write!(f, "{content}"),
        }
    }

    fn start_color(&self, f: &mut fmt::Formatter, color: u32) -> fmt::Result {
        match self.shell_kind {
            ShellKind::Xonsh => self.isolate(f, format!("\\033[{color}m")),
            // Fish needs the literal color sequences quoted: unquoted `\e[31m`
            // parses as a glob bracket expression, which fish >= 4.9 rejects
            // with "square brackets do not match". Command substitutions in the
            // prompt stay unquoted so fish executes them.
            ShellKind::Fish => self.isolate(f, format!("\"\\e[{color}m\"")),
            _ => self.isolate(f, format!("\\e[{color}m")),
        }
    }

    fn end_color(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.shell_kind {
            ShellKind::Xonsh => self.isolate(f, "\\033[0m"),
            ShellKind::Fish => self.isolate(f, "\"\\e[0m\""),
            _ => self.isolate(f, "\\e[0m"),
        }
    }
}

impl<D> fmt::Display for Color<D>
where
    D: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.start_color(f, self.color)?;
        write!(f, "{}", self.content)?;
        self.end_color(f)?;
        Ok(())
    }
}

const RED: u32 = 31;
const GREEN: u32 = 32;
const BLUE: u32 = 34;

/// Generates a PS1 string that shows the current context, namespace and depth.
///
/// Makes sure to protect the escape sequences so that the shell will not count the escape
/// sequences in the length calculation of the prompt.
pub fn generate_ps1(settings: &Settings, depth: u32, shell_kind: ShellKind) -> String {
    let current_exe_path = env::current_exe().expect("Could not get own binary path");
    let current_exe_path_str = current_exe_path.to_str().expect("Binary path is not unicode");

    let mut parts = vec![];
    parts.push(
        Color::new(
            RED,
            Command::new(format!("{current_exe_path_str} info ctx"), shell_kind),
            shell_kind,
        )
        .to_string(),
    );
    parts.push(
        Color::new(
            GREEN,
            Command::new(format!("{current_exe_path_str} info ns"), shell_kind),
            shell_kind,
        )
        .to_string(),
    );
    if settings.prompt.show_depth && depth > 1 {
        parts.push(Color::new(BLUE, depth, shell_kind).to_string());
    }

    match shell_kind {
        // Quote the surrounding brackets and separators for fish, for the same
        // reason the color sequences are quoted above.
        ShellKind::Fish => format!("\"[\"{}\"]\"", parts.join("\"|\"")),
        _ => format!("[{}]", parts.join("|")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fish_prompt_quotes_literals_but_keeps_command_substitutions_unquoted() {
        let prompt = generate_ps1(&Settings::default(), 2, ShellKind::Fish);

        // Literal segments (colors, separators, brackets) must be quoted so fish
        // >= 4.9 does not parse the unquoted prompt as a glob bracket expression.
        assert!(prompt.starts_with("\"[\""), "expected quoted '[', got: {prompt}");
        assert!(
            prompt.contains("\"\\e[31m\""),
            "expected quoted color escape, got: {prompt}"
        );

        // Command substitutions must stay unquoted so fish actually executes
        // `kubie info ctx` / `kubie info ns` instead of printing them literally.
        assert!(
            prompt.contains("info ctx)\""),
            "expected unquoted ctx command substitution, got: {prompt}"
        );
        assert!(
            prompt.contains("info ns)\""),
            "expected unquoted ns command substitution, got: {prompt}"
        );
    }

    #[test]
    fn test_non_fish_prompt_is_unaffected() {
        let prompt = generate_ps1(&Settings::default(), 2, ShellKind::Bash);
        assert!(prompt.contains("\\e[31m"), "got: {prompt}");
        assert!(!prompt.contains("\"\\e[31m\""), "got: {prompt}");
    }
}
