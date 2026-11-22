//! The treasury's own errors, numbered from 6000 in declaration order.

use naclac_lang::prelude::*;

#[error_code]
pub enum TreasuryError {
    /// The threshold must be between 1 and the member limit.
    InvalidThreshold,
    /// The treasury already has the maximum number of members.
    MembersFull,
    /// The address is already a member.
    AlreadyMember,
    /// The address is not a member.
    NotAMember,
    /// Removing the member would leave fewer members than the threshold.
    BelowThreshold,
    /// The treasury is paused.
    Paused,
    /// The treasury already has this status.
    SameStatus,
    /// The amount must be greater than zero.
    ZeroAmount,
    /// An amount overflowed.
    Overflow,
    /// The proposal id is not the treasury's next id.
    InvalidProposalId,
    /// The deadline is not in the future, or has passed.
    DeadlinePassed,
    /// The member has already approved the proposal.
    AlreadyApproved,
    /// The proposal has already been executed.
    AlreadyExecuted,
    /// The proposal has not been executed.
    NotExecuted,
    /// The proposal has fewer approvals than the threshold.
    NotEnoughApprovals,
    /// The treasury does not hold enough for this proposal.
    InsufficientFunds,
    /// The note belongs to someone else.
    NotNoteAuthority,
    /// The note does not hold enough to move this amount.
    InsufficientCount,
}
