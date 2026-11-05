//! How a job ends early: the user cancelled it, or a step failed.

use tokio::sync::watch;

pub enum Stop {
    Cancelled,
    Failed(String),
}

impl From<String> for Stop {
    fn from(e: String) -> Self {
        Stop::Failed(e)
    }
}

/// What a long transfer or remote command came back with when it watched for cancel.
pub enum Outcome<T> {
    Done(T),
    Cancelled,
}

pub type Token = watch::Receiver<bool>;

/// Resolves once the user cancels; never resolves if the job finishes first.
pub async fn requested(token: &mut Token) {
    if token.wait_for(|c| *c).await.is_err() {
        std::future::pending::<()>().await;
    }
}
