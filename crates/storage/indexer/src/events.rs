//! MIP20 event ABI definitions for log decoding.

use alloy_sol_types::sol;

sol! {
    /// MIP20 Transfer event.
    event Transfer(address indexed from, address indexed to, uint256 amount);

    /// MIP20 Transfer with memo event.
    event TransferWithMemo(address indexed from, address indexed to, uint256 amount, bytes32 indexed memo);

    /// Role membership change event from IRolesAuth.
    event RoleMembershipUpdated(bytes32 indexed role, address indexed account, address indexed sender, bool hasRole);

    /// Token creation event from MIP20Factory.
    event TokenCreated(address indexed token, string name, string symbol, string currency, address quoteToken, address admin, bytes32 salt);
}
