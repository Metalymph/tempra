use anyhow::Result;
use fero_driver::Driver;
use std::path::Path;

/// Application shell that delegates to the [`Driver`].
pub struct App {
    driver: Driver,
}

impl App {
    pub fn new() -> Self {
        Self {
            driver: Driver::new(),
        }
    }

    pub fn run(&self, file: &str) -> Result<()> {
        self.driver.run_file(Path::new(file))
    }

    pub fn check(&self, file: &str) -> Result<()> {
        self.driver.check_file(Path::new(file))
    }

    pub fn build(&self, file: &str, target: &str, mode: &str) -> Result<()> {
        println!("building {file} (target={target}, mode={mode})");
        self.driver.build_file(Path::new(file))
    }

    pub fn install(&self, file: &str) -> Result<()> {
        self.driver.install_file(Path::new(file))
    }

    pub fn test(&self) -> Result<()> {
        println!("running tests…");
        Ok(())
    }

    pub fn fmt(&self) -> Result<()> {
        println!("formatting…");
        Ok(())
    }

    pub fn repl(&self) -> Result<()> {
        println!("starting REPL…");
        Ok(())
    }
}
