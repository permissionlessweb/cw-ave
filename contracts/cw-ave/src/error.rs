use cosmwasm_std::{Instantiate2AddressError, StdError, VerificationError};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    StdError(#[from] StdError),

    #[error("{0}")]
    VerificationError(#[from] VerificationError),

    #[error("{0}")]
    Instantiate2AddressError(#[from] Instantiate2AddressError),

    #[error("Cannot set duplicate guest weights.")]
    DuplicateGuestWeight {},

    #[error("Cannot reserve more than one ticket for the same homie.")]
    DuplicateHomiesTicketAddr {},

    #[error("Cannot reserve more than 10 tickets for da homies.")]
    TooManyHomieTickets {},

    #[error("Verification has failed checking in. Ensure you are using the correct format")]
    CheckinVerificationFailed {},

    #[error("You are trying to reserve more than the limit for a single wallet.")]
    CannotReserveTicketCount {},

    #[error("NoReservedTicketsForGuest.")]
    NoReservedTicketsForGuest {},

    #[error("end of the previous event stage must come before the start of the next event stage.")]
    OverlappingEventDates {},

    #[error("event stage start must come before event stage end.")]
    InvalidEventDates {},

    #[error("IncorrectCheckinSignature")]
    IncorrectCheckinSignature {},

    #[error("Guest has already checked in .")]
    GuestHasCheckedInAllTickets {},

    #[error("No guest type exists.")]
    GuestTypeIncorrect {},

    #[error("you cannot register, this wallet is not an usher for this event.")]
    NotAnEventUsher {},

    #[error("DuplicateFeeDenom.")]
    DuplicateFeeDenom {},

    #[error("yBadEventDescriptionLengtht.")]
    BadEventDescriptionLength {},

    #[error("the token set as being used for ticket payment was not found in sent tokens..")]
    GuestTicketPaymentSetIncorrect {},

    #[error("you did not send the required amount of funds for the ticket payment.")]
    NotEnoughtFundsSetForTicketPayment {},

    #[error("BadEventTitle.")]
    BadEventTitleOrDescription {},

    #[error("BadGuestDetailParams.")]
    BadGuestDetailParams {},

    #[error("guest has already checked in.")]
    GuestAlreadyCheckedIn {},

    #[error("this guest is not allowed to checkin for this specific event segment.")]
    IncorrectEventSegmentId {},
}
