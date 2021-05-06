mod cli;
mod daemon;
mod logger;

use cli::CLI;
use structopt::StructOpt;

#[async_std::main]
async fn main() {
    logger::setup_logger();
    // Capture CLI inputs
    match CLI::from_args() {
        CLI { daemon_opts } => daemon::start(daemon_opts.to_config().unwrap()).await,
    }
}
