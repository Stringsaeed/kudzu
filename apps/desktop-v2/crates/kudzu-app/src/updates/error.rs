#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum UpdateError {
    #[error("Updates are unavailable in Kudzu Dev.")]
    Dev,
    #[error("Move Kudzu to your Applications folder so it can update itself.")]
    NeedsMove,
    #[error("The update check failed.")]
    Check,
    #[error("The update could not be downloaded.")]
    Download,
    #[error("The update is not signed correctly.")]
    Signature,
    #[error("The update information could not be read.")]
    Feed,
    #[error("This update could not be installed.")]
    Install,
    #[error("The timer could not start.")]
    Schedule,
}
