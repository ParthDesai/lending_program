use crate::{error::LendingPlatformError, processor::Processor};
use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult,
    program_error::PrintProgramError, pubkey::Pubkey,
};

entrypoint!(process_instruction);
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    println!("Adding additional changes here");
    println!("Also changes are multiline");
    if let Err(error) = Processor::process(program_id, accounts, instruction_data) {
        error.print::<LendingPlatformError>();
        return Err(error);
    }
    Ok(())
}
