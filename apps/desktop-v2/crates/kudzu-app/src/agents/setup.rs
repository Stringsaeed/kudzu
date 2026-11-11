//! Reviewable client configuration and explicit skill installation.
use super::{AgentError, AgentsService, SKILL};
use serde_json::json;
use std::{fs, io::Write, os::unix::fs::OpenOptionsExt, path::PathBuf};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillState {
    Missing,
    Current,
    Outdated,
}
#[derive(Debug)]
pub struct AgentSetup {
    pub running: bool,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub claude_command: String,
    pub config_json: String,
    pub skill_path: PathBuf,
    pub skill: SkillState,
    pub bundled_skill_dir: Option<PathBuf>,
}
impl AgentsService {
    pub fn setup(&self) -> AgentSetup {
        let c = &self.config;
        let command = c.executable.to_string_lossy().into_owned();
        let skill_path = c.home.join(".claude/skills/kudzu/SKILL.md");
        let skill = match fs::read_to_string(&skill_path) {
            Ok(s) if s == SKILL => SkillState::Current,
            Ok(_) => SkillState::Outdated,
            Err(_) => SkillState::Missing,
        };
        AgentSetup {
            running: self.status().get().running,
            name: c.name.clone(),
            command: command.clone(),
            args: c.args.clone(),
            claude_command: format!(
                "claude mcp add --scope user {} -- {} {}",
                shell_quote(&c.name),
                shell_quote(&command),
                c.args
                    .iter()
                    .map(|a| shell_quote(a))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            config_json: serde_json::to_string_pretty(
                &json!({"mcpServers":{&c.name:{"command":command,"args":c.args}}}),
            )
            .unwrap_or_default(),
            skill_path,
            skill,
            bundled_skill_dir: c.bundled_skill_dir.clone(),
        }
    }
    /// Atomically replace the destination, including a symlink, with a regular 0600 file.
    /// Sync the file before rename, then sync its containing directory.
    pub fn install_skill(&self) -> Result<PathBuf, AgentError> {
        let path = self.setup().skill_path;
        fs::create_dir_all(path.parent().expect("skill parent"))?;
        let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> std::io::Result<()> {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)?;
            file.write_all(SKILL.as_bytes())?;
            file.sync_all()?;
            fs::rename(&temporary, &path)?;
            fs::File::open(path.parent().expect("skill parent"))?.sync_all()
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
        Ok(path)
    }
}
fn shell_quote(s: &str) -> String {
    if !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/._-+=:,@".contains(&b))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quotes_paths_for_the_shell() {
        assert_eq!(
            shell_quote("/Applications/Kudzu.app/Contents/MacOS/Kudzu"),
            "/Applications/Kudzu.app/Contents/MacOS/Kudzu"
        );
        assert_eq!(
            shell_quote("/Users/a b/Kudzu Dev.app/x"),
            "'/Users/a b/Kudzu Dev.app/x'"
        );
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }
}
