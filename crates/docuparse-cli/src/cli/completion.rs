use usage_rs::{Args, Run};

use super::DocuparseCli;

/// Print a shell completion script
#[derive(Args)]
pub(crate) struct Completion {
    /// Which shell to generate for
    #[usage(long, choices("bash", "zsh", "fish"))]
    shell: String,
}

impl Run for Completion {
    type Output = ();

    fn run(self) {
        let shell = match self.shell.as_str() {
            "bash" => usage_rs::complete::Shell::Bash,
            "zsh" => usage_rs::complete::Shell::Zsh,
            _ => usage_rs::complete::Shell::Fish,
        };
        print!("{}", DocuparseCli::completion_script(shell));
    }
}
