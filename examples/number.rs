
use anyhow::Result;
use parallel_runner::Container;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let mut runner = Container::new(
        5,
        &|| { log::info!("asdffs"); std::thread::sleep(std::time::Duration::from_secs(1)); 4 },
        &|item| { log::info!("{}", item); }
    )?;
    runner.run().await?;
    Ok(())
}