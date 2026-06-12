use crate::errors::*;
use std::str::FromStr;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub struct Prompt {
    stdout: tokio::io::Stdout,
    stdin: BufReader<tokio::io::Stdin>,
    buf: String,
}

// Make clippy happy
impl Default for Prompt {
    fn default() -> Self {
        Self::new()
    }
}

impl Prompt {
    pub fn new() -> Self {
        Self {
            stdout: tokio::io::stdout(),
            stdin: BufReader::new(tokio::io::stdin()),
            buf: String::new(),
        }
    }

    pub async fn get<T: FromStr>(&mut self, question: &str) -> Result<T>
    where
        T::Err: std::fmt::Display,
    {
        loop {
            self.stdout.write_all(question.as_bytes()).await?;
            self.stdout.flush().await?;

            self.buf.clear();
            self.stdin.read_line(&mut self.buf).await?;

            if self.buf.is_empty() {
                self.stdout.write_all(b"\n").await?;
                bail!("Stdin has been closed");
            }

            let input = self.buf.trim_end();

            if input.is_empty() {
                continue;
            }

            match input.parse::<T>() {
                Ok(value) => return Ok(value),
                Err(e) => {
                    self.stdout
                        .write_all(format!("Error: {}\n", e).as_bytes())
                        .await?;
                }
            }
        }
    }
}

pub enum YesNo {
    Yes,
    No,
}

impl YesNo {
    pub fn is_yes(&self) -> bool {
        matches!(self, YesNo::Yes)
    }
}

impl FromStr for YesNo {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "yes" => Ok(YesNo::Yes),
            "no" => Ok(YesNo::No),
            _ => bail!("Please enter 'yes' or 'no'"),
        }
    }
}
