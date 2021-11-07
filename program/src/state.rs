use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use solana_program::program_error::ProgramError;
use solana_program::program_option::COption;
use solana_program::program_pack::{IsInitialized, Pack, Sealed};
use solana_program::pubkey::Pubkey;

pub fn pack_coption_key(src: &COption<Pubkey>, dst: &mut [u8; 36]) {
    let (tag, body) = mut_array_refs![dst, 4, 32];
    match src {
        COption::Some(key) => {
            *tag = [1, 0, 0, 0];
            body.copy_from_slice(key.as_ref());
        }
        COption::None => {
            *tag = [0; 4];
        }
    }
}

pub fn unpack_coption_key(src: &[u8; 36]) -> Result<COption<Pubkey>, ProgramError> {
    let (tag, body) = array_refs![src, 4, 32];
    match *tag {
        [0, 0, 0, 0] => Ok(COption::None),
        [1, 0, 0, 0] => Ok(COption::Some(Pubkey::new_from_array(*body))),
        _ => Err(ProgramError::InvalidAccountData),
    }
}

pub const LENDINGPOOL_OPEN: u8 = 1;
pub const LENDINGPOOL_CLOSE: u8 = 2;

/// This will hold lender's stable coin, both original and interest
#[derive(PartialEq, Debug, Default)]
pub struct LendingPoolState {
    pub initialized: bool,
    pub number_of_outstanding_loans: u64,
    pub status: u8,
    pub expected_apy: u8,
    pub max_payback_time: u64,
    pub lender: Pubkey,
    pub chainlink_feed_account: Pubkey,
    pub collateral_amount: u128,
    pub coin_amount: u128
}

impl IsInitialized for LendingPoolState {
    fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Sealed for LendingPoolState{}

impl Pack for LendingPoolState {
    const LEN: usize = 115;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, LendingPoolState::LEN];

        let (
            initialized_dst,
            number_of_outstanding_loans_dst,
            status_dst,
            expected_apy_dst,
            max_payback_time_dst,
            lender_dst,
            chainlink_feed_account_dst,
            collateral_amount_dst,
            coin_amount_dst
        ) = mut_array_refs![dst, 1, 8, 1, 1, 8, 32, 32, 16, 16];

        initialized_dst[0] = self.initialized as u8;
        number_of_outstanding_loans_dst.copy_from_slice(&self.number_of_outstanding_loans.to_le_bytes());
        status_dst.copy_from_slice(&self.status.to_le_bytes());
        expected_apy_dst.copy_from_slice(&self.expected_apy.to_le_bytes());
        max_payback_time_dst.copy_from_slice(&self.max_payback_time.to_le_bytes());
        lender_dst.copy_from_slice(&self.lender.to_bytes());
        chainlink_feed_account_dst.copy_from_slice(&self.chainlink_feed_account.to_bytes());
        collateral_amount_dst.copy_from_slice(&self.collateral_amount.to_le_bytes());
        coin_amount_dst.copy_from_slice(&self.coin_amount.to_le_bytes());
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, LendingPoolState::LEN];

        let (
            initialized_src,
            number_of_outstanding_loans_src,
            status_src,
            expected_apy_src,
            max_payback_time_src,
            lender_src,
            chainlink_feed_account_src,
            collateral_amount_src,
            coin_amount_src
        ) = array_refs![src, 1, 8, 1, 1, 8, 32, 32, 16, 16];

        let is_initialized = match initialized_src {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let number_of_outstanding_loans = u64::from_le_bytes(*number_of_outstanding_loans_src);
        let status = u8::from_le_bytes(*status_src);
        let expected_apy = u8::from_le_bytes(*expected_apy_src);
        let max_payback_time = u64::from_le_bytes(*max_payback_time_src);
        let lender = Pubkey::new_from_array(*lender_src);
        let chainlink_feed_account = Pubkey::new_from_array(*chainlink_feed_account_src);
        let collateral_amount = u128::from_le_bytes(*collateral_amount_src);
        let coin_amount = u128::from_le_bytes(*coin_amount_src);

        Ok(LendingPoolState{
            initialized: is_initialized,
            number_of_outstanding_loans,
            status,
            expected_apy,
            max_payback_time,
            lender,
            chainlink_feed_account,
            collateral_amount,
            coin_amount
        })
    }
}

pub const LOANSTATE_LOANED: u8 = 1;
pub const LOANSTATE_PAYEDBACK: u8 = 2;
pub const LOANSTATE_DEFAULTED: u8 = 3;

/// This will hold borrower's collateral, which will be released
/// upon payback. If borrower does not pay back on specified time
/// the collateral will be transferred to owner
#[derive(PartialEq, Debug, Default)]
pub struct LoanState {
    pub initialized: bool,
    pub status: u8,
    pub amount: u64,
    pub lending_account: Pubkey,
    // This should be spl account and mint should be equal to lending pool's payback address's mint
    pub borrower: Pubkey,
    pub borrowed_on: u64,
    pub validity_till: u64,
    pub expected_apy: u8,
    pub collateral_lamports: u128
}

impl IsInitialized for LoanState {
    fn is_initialized(&self) -> bool {
        self.initialized
    }
}

impl Sealed for LoanState{}

impl Pack for LoanState {
    const LEN: usize = 107;

    fn pack_into_slice(&self, dst: &mut [u8]) {
        let dst = array_mut_ref![dst, 0, LoanState::LEN];

        let (
            initialized_dst,
            status_dst,
            amount_dst,
            lending_account_dst,
            borrower_dst,
            borrowed_on_dst,
            validity_till_dst,
            expected_apy_dst,
            collateral_lamports_dst
        ) = mut_array_refs![dst, 1, 1, 8, 32, 32, 8, 8, 1, 16];

        initialized_dst[0] = self.initialized as u8;
        status_dst.copy_from_slice(&self.status.to_le_bytes());
        amount_dst.copy_from_slice(&self.amount.to_le_bytes());
        lending_account_dst.copy_from_slice(&self.lending_account.to_bytes());
        borrower_dst.copy_from_slice(&self.borrower.to_bytes());
        borrowed_on_dst.copy_from_slice(&self.borrowed_on.to_le_bytes());
        validity_till_dst.copy_from_slice(&self.validity_till.to_le_bytes());
        expected_apy_dst.copy_from_slice(&self.expected_apy.to_le_bytes());
        collateral_lamports_dst.copy_from_slice(&self.collateral_lamports.to_le_bytes());
    }

    fn unpack_from_slice(src: &[u8]) -> Result<Self, ProgramError> {
        let src = array_ref![src, 0, LoanState::LEN];

        let (
            initialized_src,
            status_src,
            amount_src,
            lending_account_src,
            borrower_src,
            borrowed_on_src,
            validity_till_src,
            expected_apy_src,
            collateral_lamports_src
        ) = array_refs![src, 1, 1, 8, 32, 32, 8, 8, 1, 16];

        let is_initialized = match initialized_src {
            [0] => false,
            [1] => true,
            _ => return Err(ProgramError::InvalidAccountData),
        };

        let status = u8::from_le_bytes(*status_src);
        let amount = u64::from_le_bytes(*amount_src);
        let lending_account = Pubkey::new_from_array(*lending_account_src);
        let borrower = Pubkey::new_from_array(*borrower_src);
        let borrowed_on = u64::from_le_bytes(*borrowed_on_src);
        let validity_till = u64::from_le_bytes(*validity_till_src);
        let expected_apy = u8::from_le_bytes(*expected_apy_src);
        let collateral_lamports = u128::from_le_bytes(*collateral_lamports_src);

        
        Ok(LoanState{
            initialized: is_initialized,
            status,
            amount,
            lending_account,
            borrower,
            borrowed_on,
            validity_till,
            expected_apy,
            collateral_lamports
        })
    }
}
