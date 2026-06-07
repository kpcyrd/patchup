use crate::errors::*;

fn landlock() -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        use landlock::{
            Access, AccessFs, Ruleset, RulesetAttr, RulesetCreatedAttr, RulesetStatus,
            path_beneath_rules,
        };
        use std::env;

        let path = env::current_dir().context("Failed to determine current directory")?;

        let abi = landlock::ABI::V1;
        let status = Ruleset::default()
            .handle_access(AccessFs::from_all(abi))?
            .create()?
            .add_rules(path_beneath_rules(&[path], AccessFs::from_all(abi)))?
            .restrict_self()?;

        match status.ruleset {
            RulesetStatus::FullyEnforced => info!("Successfully enabled landlock rules"),
            RulesetStatus::PartiallyEnforced => {
                warn!("Partially enabled landlock rules, please update your kernel")
            }
            RulesetStatus::NotEnforced => bail!("Could not enforce, please update your kernel"),
        }
    }

    Ok(())
}

pub fn init() {
    if let Err(err) = landlock() {
        // This is intentionally not a fatal error, so you can run the agent on kernels
        // without landlock support.
        warn!("Failed to set up landlock: {err:#}");
    }
}
