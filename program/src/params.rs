use crate::error::LendingPlatformError;
use arrayref::{array_ref, array_refs, mut_array_refs};
use solana_program::program_error::ProgramError;

#[derive(Debug, PartialEq)]
pub struct InitLoanAccount {
    pub loan_bump_seed: u8
}

impl InitLoanAccount {
    const LEN: usize = 1;

    pub fn len(&self) -> usize {
        Self::LEN
    }

    pub fn unpack(instruction_data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if instruction_data.len() < InitLoanAccount::LEN {
            return Err(LendingPlatformError::InvalidInstruction.into());
        }

        let (data, rest) = instruction_data.split_at(InitLoanAccount::LEN);
        let src = array_ref![data, 0, InitLoanAccount::LEN];

        let bump_seed = src[0];

        Ok((InitLoanAccount{
            loan_bump_seed: bump_seed
        }, rest))
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut dst = [0u8; InitLoanAccount::LEN];

        dst[0] = self.loan_bump_seed;

        dst.to_vec()
    }

}

#[derive(Debug, PartialEq)]
pub struct InitLendingPoolAccount {
    pub bump_seed: u8
}

impl InitLendingPoolAccount {
    const LEN: usize = 1;

    pub fn len(&self) -> usize {
        Self::LEN
    }

    pub fn unpack(instruction_data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if instruction_data.len() < InitLendingPoolAccount::LEN {
            return Err(LendingPlatformError::InvalidInstruction.into());
        }

        let (data, rest) = instruction_data.split_at(InitLendingPoolAccount::LEN);
        let src = array_ref![data, 0, InitLendingPoolAccount::LEN];

        let bump_seed = src[0];

        Ok((InitLendingPoolAccount{
            bump_seed
        }, rest))
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut dst = [0u8; InitLendingPoolAccount::LEN];

        dst[0] = self.bump_seed;

        dst.to_vec()
    }

}

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct NewLendingPool {
    pub total_lending_amount: u64,
    pub max_payback_time: u64,
    pub expected_apy: u8,
    pub bump_seed: u8
}

impl NewLendingPool {
    const LEN: usize = 18;

    pub fn len(&self) -> usize {
        Self::LEN
    }

    pub fn unpack(instruction_data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if instruction_data.len() < NewLendingPool::LEN {
            return Err(LendingPlatformError::InvalidInstruction.into());
        }

        let (data, rest) = instruction_data.split_at(NewLendingPool::LEN);
        let src = array_ref![data, 0, NewLendingPool::LEN];
        let (total_lending_amt_src, max_payback_time_src, expected_apy_src, bump_seed_src) =
            array_refs![src, 8, 8, 1, 1];

        let total_lending_amt = u64::from_le_bytes(*total_lending_amt_src);
        let max_payback_time = u64::from_le_bytes(*max_payback_time_src);
        let expected_apy = u8::from_le_bytes(*expected_apy_src);
        let bump_seed = bump_seed_src[0];

        Ok((NewLendingPool{
            total_lending_amount: total_lending_amt,
            max_payback_time,
            expected_apy,
            bump_seed
        }, rest))
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut dst = [0u8; NewLendingPool::LEN];

        let (total_lending_amt_dst, max_payback_time_dst, expected_apy_dst, bump_seed_dst) =
            mut_array_refs![&mut dst, 8, 8, 1, 1];

        total_lending_amt_dst.copy_from_slice(&self.total_lending_amount.to_le_bytes());
        expected_apy_dst.copy_from_slice(&self.expected_apy.to_le_bytes());
        max_payback_time_dst.copy_from_slice(&self.max_payback_time.to_le_bytes());
        bump_seed_dst[0] = self.bump_seed;

        dst.to_vec()
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct NewLoan {
    pub amount: u64,
    pub lending_pool_bump_seed: u8,
    pub loan_bump_seed: u8,
    pub loan_collateral_bump_seed: u8
}

impl NewLoan {
    const LEN: usize = 11;

    pub fn len(&self) -> usize {
        Self::LEN
    }

    pub fn unpack(instruction_data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if instruction_data.len() < NewLoan::LEN {
            return Err(LendingPlatformError::InvalidInstruction.into());
        }

        let (data, rest) = instruction_data.split_at(NewLoan::LEN);
        let src = array_ref![data, 0, NewLoan::LEN];

        let (amount_src, lending_pool_bump_seed_src, loan_bump_seed_src, loan_collateral_bump_seed_src) = array_refs![src, 8, 1, 1, 1];

        let amount = u64::from_le_bytes(*amount_src);
        let lending_pool_bump_seed = lending_pool_bump_seed_src[0];
        let loan_bump_seed = loan_bump_seed_src[0];
        let loan_collateral_bump_seed = loan_collateral_bump_seed_src[0];


        Ok((NewLoan{
            amount,
            lending_pool_bump_seed,
            loan_bump_seed,
            loan_collateral_bump_seed
        }, rest))
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut dst = [0u8; NewLoan::LEN];

        let (amount_dest, lending_pool_bump_seed_dst, loan_bump_seed_dst, loan_collateral_bump_seed_dst) = mut_array_refs![&mut dst, 8, 1, 1, 1];

        amount_dest.copy_from_slice(&self.amount.to_le_bytes());
        lending_pool_bump_seed_dst[0] = self.lending_pool_bump_seed;
        loan_bump_seed_dst[0] = self.loan_bump_seed;
        loan_collateral_bump_seed_dst[0] = self.loan_collateral_bump_seed;

        dst.to_vec()
    }
}

#[derive(Debug, PartialEq)]
pub struct PaybackLoan {
    pub lending_pool_bump_seed: u8,
    pub loan_bump_seed: u8,
    pub loan_collateral_bump_seed: u8
}


impl PaybackLoan {
    const LEN: usize = 3;

    pub fn len(&self) -> usize {
        Self::LEN
    }

    pub fn unpack(instruction_data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if instruction_data.len() < PaybackLoan::LEN {
            return Err(LendingPlatformError::InvalidInstruction.into());
        }

        let (data, rest) = instruction_data.split_at(PaybackLoan::LEN);
        let src = array_ref![data, 0, PaybackLoan::LEN];

        let lending_pool_bump_seed = src[0];
        let loan_bump_seed = src[1];
        let loan_collateral_bump_seed = src[2];

        Ok((PaybackLoan {
            lending_pool_bump_seed,
            loan_bump_seed,
            loan_collateral_bump_seed
        }, rest))
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut dst = [0u8; PaybackLoan::LEN];

        dst[0] = self.lending_pool_bump_seed;
        dst[1] = self.loan_bump_seed;
        dst[2] = self.loan_collateral_bump_seed;

        dst.to_vec()
    }
}

#[derive(Debug, PartialEq)]
pub struct DefaultLoan {
    pub lending_pool_bump_seed: u8,
    pub loan_bump_seed: u8,
    pub lending_pool_collateral_seed: u8,
    pub loan_collateral_seed: u8
}

impl DefaultLoan {
    const LEN: usize = 4;

    pub fn len(&self) -> usize {
        Self::LEN
    }

    pub fn unpack(instruction_data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if instruction_data.len() < DefaultLoan::LEN {
            return Err(LendingPlatformError::InvalidInstruction.into());
        }

        let (data, rest) = instruction_data.split_at(DefaultLoan::LEN);
        let src = array_ref![data, 0, DefaultLoan::LEN];

        let lending_pool_bump_seed = src[0];
        let loan_bump_seed = src[1];
        let lending_pool_collateral_seed = src[2];
        let loan_collateral_seed = src[3];

        Ok((DefaultLoan {
            lending_pool_bump_seed,
            loan_bump_seed,
            lending_pool_collateral_seed,
            loan_collateral_seed
        }, rest))
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut dst = [0u8; DefaultLoan::LEN];

        dst[0] = self.lending_pool_bump_seed;
        dst[1] = self.loan_bump_seed;
        dst[2] = self.lending_pool_collateral_seed;
        dst[3] = self.loan_collateral_seed;

        dst.to_vec()
    }
}

#[derive(Debug, PartialEq)]
pub struct CloseLending {
    pub lending_pool_bump_seed: u8,
    pub lending_pool_collateral_seed: u8
}

impl CloseLending {
    const LEN: usize = 2;

    pub fn len(&self) -> usize {
        Self::LEN
    }

    pub fn unpack(instruction_data: &[u8]) -> Result<(Self, &[u8]), ProgramError> {
        if instruction_data.len() < CloseLending::LEN {
            return Err(LendingPlatformError::InvalidInstruction.into());
        }

        let (data, rest) = instruction_data.split_at(CloseLending::LEN);
        let src = array_ref![data, 0, CloseLending::LEN];

        let lending_pool_bump_seed = src[0];
        let lending_pool_collateral_seed = src[1];

        Ok((CloseLending {
            lending_pool_bump_seed,
            lending_pool_collateral_seed
        }, rest))
    }

    pub fn pack(&self) -> Vec<u8> {
        let mut dst = [0u8; CloseLending::LEN];

        dst[0] = self.lending_pool_bump_seed;
        dst[1] = self.lending_pool_collateral_seed;

        dst.to_vec()
    }
}


