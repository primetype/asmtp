use console::{style, Emoji, Style as ConsoleStyle};
use dialoguer::theme::ColorfulTheme;
use indicatif::ProgressStyle;

pub struct Style {
    pub dialoguer: ColorfulTheme,
    pub spinner_interval: u64,
    pub spinner: ProgressStyle,
    pub passport: ConsoleStyle,
    pub public_key: ConsoleStyle,
    pub topic: ConsoleStyle,
    pub alias: ConsoleStyle,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            dialoguer: ColorfulTheme {
                defaults_style: ConsoleStyle::new().for_stderr().cyan(),
                prompt_style: ConsoleStyle::new().for_stderr().bold(),
                prompt_prefix: style("?".to_string()).for_stderr().yellow(),
                prompt_suffix: style(Emoji("›", ">").to_string())
                    .for_stderr()
                    .black()
                    .bright(),
                success_prefix: style(Emoji("✔", "").to_string()).for_stderr().green(),
                success_suffix: style(Emoji("·", "").to_string())
                    .for_stderr()
                    .black()
                    .bright(),
                error_prefix: style(Emoji("✘", "").to_string()).for_stderr().red(),
                error_style: ConsoleStyle::new().for_stderr().red(),
                hint_style: ConsoleStyle::new().for_stderr().black().bright(),
                values_style: ConsoleStyle::new().for_stderr().green(),
                active_item_style: ConsoleStyle::new().for_stderr().cyan(),
                inactive_item_style: ConsoleStyle::new().for_stderr(),
                active_item_prefix: style(Emoji("❯", ">").to_string()).for_stderr().green(),
                inactive_item_prefix: style(" ".to_string()).for_stderr(),
                checked_item_prefix: style(Emoji("✔", "").to_string()).for_stderr().green(),
                unchecked_item_prefix: style(Emoji("✔", "").to_string()).for_stderr().black(),
                picked_item_prefix: style(Emoji("❯", ">").to_string()).for_stderr().green(),
                unpicked_item_prefix: style(" ".to_string()).for_stderr(),
                inline_selections: true,
            },
            spinner_interval: 175,
            spinner: ProgressStyle::default_spinner()
                .template("{spinner:.cyan} {msg}")
                .tick_strings(&["●∙∙∙", "∙●∙∙", "∙∙●∙", "∙∙∙●", "∙∙●∙", "∙●∙∙", "●∙∙∙"]),
            passport: ConsoleStyle::new().for_stdout().cyan().bold(),
            public_key: ConsoleStyle::new().for_stdout().green().bright(),
            topic: ConsoleStyle::new().for_stdout().red().bright().italic(),
            alias: ConsoleStyle::new().for_stdout().bold(),
        }
    }
}
