use clap::Parser;

#[derive(Debug, Clone, Parser)]
pub struct Args {
    #[clap(subcommand)]
    pub subcommand: Subcommand,
}

#[derive(Debug, Clone, Parser)]
pub enum Subcommand {
    Install(Install),
    Uninstall(Uninstall),
}

#[derive(Debug, Clone, Parser)]
pub struct Install {}

#[derive(Debug, Clone, Parser)]
pub struct Uninstall {}
