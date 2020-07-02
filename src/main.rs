use node_cli::Cli;
use sc_cli::{build_runtime, SubstrateCli};
use sc_service::TaskType;
use substrate_test_runner::node::build_node;

fn main () -> Result<(), sc_cli::Error> {
	use sc_cli::SubstrateCli;
	let cli = node_cli::Cli::from_args();

	match &cli.subcommand {
		Some(subcommand) => {
			let runner = cli.create_runner(subcommand)?;
			runner.run_subcommand(subcommand, |config| Ok(new_full_start!(config).0))
		}
		None => {
			let runner = cli.create_runner(&cli.run)?;
			runner.run_node_until_exit(config| {
				build_node(config)
					.map(|(taskmanager, rpc)| {
						taskmanager
					})
			})
		}
	}
	let config = cli
		.create_runner(&cli.run)
		// Todo: return result
		.unwrap();
	config.run_node_until_exit(|config| {
		build_node(config)
			.map(|(taskmanager, rpc)| {
				taskmanager
			})
	})
}
