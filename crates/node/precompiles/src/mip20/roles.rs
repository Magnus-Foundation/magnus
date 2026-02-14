//! Role-based access control for MIP20 tokens.

/// Token roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Role {
    /// Full admin: can grant/revoke all other roles.
    Admin = 0,
    /// Can mint new tokens.
    Minter = 1,
    /// Can burn tokens.
    Burner = 2,
    /// Can pause transfers.
    Pauser = 3,
}
