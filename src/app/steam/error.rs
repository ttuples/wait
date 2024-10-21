

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginError {
    AlreadyLoggedIn,
    Other(String),
}

impl std::fmt::Display for LoginError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            LoginError::AlreadyLoggedIn => write!(f, "Already logged in"),
            LoginError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for LoginError {}