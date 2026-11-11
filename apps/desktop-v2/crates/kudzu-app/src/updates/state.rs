/// Percent shown when the download size is unknown (About pane 30% bar).
pub const UNKNOWN_TOTAL_PERCENT: u64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    Checking,
    UpToDate {
        checked_at: String,
    },
    Downloading {
        version: String,
        downloaded: u64,
        total: Option<u64>,
    },
    Ready {
        version: String,
        notes: Option<String>,
        date: Option<String>,
    },
    Restarting {
        version: String,
    },
    Error {
        message: String,
        needs_move: bool,
    },
}

impl UpdateState {
    pub fn display_percent(&self) -> Option<u64> {
        match self {
            Self::Downloading {
                downloaded, total, ..
            } => Some(display_percent(*downloaded, *total)),
            _ => None,
        }
    }
}

pub fn display_percent(downloaded: u64, total: Option<u64>) -> u64 {
    match total {
        Some(total) if total > 0 => (downloaded.saturating_mul(100) / total).min(100),
        _ => UNKNOWN_TOTAL_PERCENT,
    }
}
