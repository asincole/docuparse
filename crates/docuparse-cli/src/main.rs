mod cli;

use cli::DocuparseCli;
use usage_rs::Run;

#[hotpath::main]
fn main() {
    DocuparseCli::parse().command.run()
}
