use anyhow::Result;

pub fn run_update(_force: bool) -> Result<bool> {
    println!("thesmos is updated with Homebrew: brew upgrade thesmos");
    Ok(false)
}
