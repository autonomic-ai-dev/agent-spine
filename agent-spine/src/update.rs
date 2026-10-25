use anyhow::Result;

pub fn run_update(_force: bool) -> Result<bool> {
    println!("agent-spine is updated with Homebrew: brew upgrade agent-spine");
    Ok(false)
}
