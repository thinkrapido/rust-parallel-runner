
use anyhow::Result;
use parallel_runner::Container;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    let mut runner = Container::new(
        5,
        Some(10),
        &|| { std::thread::sleep(std::time::Duration::from_secs(1)); 4 },
        &|_item| { }
    )?;
    runner.run().await?;
    Ok(())
}