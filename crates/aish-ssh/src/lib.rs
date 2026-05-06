//! aish-ssh — SSH 客户端层（russh 包装）。

#![allow(dead_code)]

mod error;

pub use error::{SshError, SshErrorKind};

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(2 + 2, 4);
    }
}
