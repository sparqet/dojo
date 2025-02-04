use clap::{Parser, Subcommand};

use self::build::BuildArgs;
use self::init::InitArgs;
use self::migrate::MigrateArgs;
use self::test::TestArgs;

pub(crate) mod build;
pub(crate) mod init;
pub(crate) mod migrate;
pub(crate) mod test;

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Build the world, generating the necessary artifacts for deployment")]
    Build(BuildArgs),
    #[command(about = "Initialize a new project")]
    Init(InitArgs),
    #[command(about = "Run a migration, declaring and deploying contracts as necessary to \
                       update the world")]
    Migrate(MigrateArgs),
    #[command(about = "Test the project's smart contracts")]
    Test(TestArgs),
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct App {
    #[command(subcommand)]
    pub command: Commands,
}
