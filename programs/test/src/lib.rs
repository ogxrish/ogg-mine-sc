use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount, transfer, Transfer};
use anchor_lang::solana_program::native_token::LAMPORTS_PER_SOL;

declare_id!("AkRNJmwe6SdYDZmtDrp4XRjYtKdMJx7bvr2xBDhbwn28");

const EPOCH_LENGTH: u64 = 10; // 86400; // one day in seconds
const STARTING_REWARD: u64 = 10_000_000;
const CREATOR: &str = "58V6myLoy5EVJA3U2wPdRDMUXpkwg8Vfw5b6fHqi2mEj";
#[program]
pub mod test {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.global_account.epoch = 0;
        ctx.accounts.global_account.epoch_end = 0;
        ctx.accounts.global_account.token_decimals = ctx.accounts.mint.decimals as u64;
        ctx.accounts.global_account.reward = 0;
        Ok(())
    }
    pub fn fund_program_token(ctx: Context<FundProgramToken>, amount: u64) -> Result<()> {
        transfer(
            CpiContext::new(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.signer_token_account.to_account_info(),
                    to: ctx.accounts.program_token_account.to_account_info(),
                    authority: ctx.accounts.signer.to_account_info()
                }
            ),
            amount
        )?;
        Ok(())
    }
    pub fn withdraw_fees(ctx: Context<WithdrawFees>, amount: u64) -> Result<()> {
        if CREATOR.parse::<Pubkey>().unwrap() != ctx.accounts.signer.key() {
            return Err(CustomError::WrongSigner.into())
        }
        **ctx.accounts.program_authority.try_borrow_mut_lamports()? -= amount;
        **ctx.accounts.signer.try_borrow_mut_lamports()? += amount;
        Ok(())
    }
    pub fn new_epoch(ctx: Context<NewEpoch>, epoch: u64) -> Result<()> {
        let time = Clock::get()?.unix_timestamp as u64;
        if time < ctx.accounts.global_account.epoch_end {
            return Err(CustomError::EpochNotOver.into())
        }
        if epoch != ctx.accounts.global_account.epoch + 1 {
            return Err(CustomError::WrongEpochProvided.into())
        }
        ctx.accounts.global_account.epoch += 1;
        ctx.accounts.global_account.epoch_end = time + EPOCH_LENGTH;
        ctx.accounts.epoch_account.total_miners = 0;
        ctx.accounts.global_account.reward = if epoch == 1 {
            STARTING_REWARD
        } else {
            ctx.accounts.global_account.reward * 7 / 8
        };
        Ok(())
    }
    pub fn mine(ctx: Context<Mine>, epoch: u64) -> Result<()> {
        let time = Clock::get()?.unix_timestamp as u64;
        if time >= ctx.accounts.global_account.epoch_end {
            return Err(CustomError::EpochOver.into())
        }
        if epoch != ctx.accounts.global_account.epoch {
            return Err(CustomError::WrongEpochProvided.into())
        }
        let price = LAMPORTS_PER_SOL / 10 / 2000 * ctx.accounts.epoch_account.total_miners.pow(2); // y (price) = .1 SOL / 2000 * x ** 2 (minters);
        ctx.accounts.epoch_account.total_miners += 1;
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.signer.to_account_info(),
                    to: ctx.accounts.program_authority.to_account_info(),
                }
            ),
            price,
        )?;
        Ok(())
    }
    pub fn claim(ctx: Context<Claim>, epoch: u64) -> Result<()> {
        if epoch >= ctx.accounts.global_account.epoch {
            return Err(CustomError::InvalidEpoch.into())
        }
        transfer(
            CpiContext::new_with_signer(
                ctx.accounts.program_authority.to_account_info(),
                Transfer {
                    from: ctx.accounts.program_token_account.to_account_info(),
                    to: ctx.accounts.signer_token_account.to_account_info(),
                    authority: ctx.accounts.program_authority.to_account_info()
                },
                &[&[b"auth", &[ctx.bumps.program_authority]]]
            ),
            ctx.accounts.global_account.reward / ctx.accounts.epoch_account.total_miners
        )?;
        Ok(())
    }
}
#[error_code]
pub enum CustomError {
    #[msg("Epoch not over")]
    EpochNotOver,
    #[msg("Wrong epoch provided")]
    WrongEpochProvided,
    #[msg("Epoch over")]
    EpochOver,
    #[msg("Wrong signer")]
    WrongSigner,
    #[msg("Invalid epoch")]
    InvalidEpoch
}
#[account]
pub struct GlobalDataAccount {
    pub epoch: u64,
    pub epoch_end: u64,
    pub token_decimals: u64,
    pub reward: u64,
}
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    pub mint: Account<'info, Mint>,
    #[account(
        init,
        payer = signer,
        seeds = [b"token_account"],
        bump,
        token::mint = mint,
        token::authority = program_authority
    )]
    pub program_token_account: Account<'info, TokenAccount>,
    #[account(
        init,
        payer = signer,
        seeds = [b"auth"],
        bump,
        space = 8,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    #[account(
        init,
        payer = signer,
        seeds = [b"global"],
        bump,
        space = 8 + 8 + 8 + 8
    )]
    pub global_account: Account<'info, GlobalDataAccount>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct FundProgramToken<'info> {
    pub signer: Signer<'info>,
    #[account(mut)]
    pub signer_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"token_account"],
        bump,
    )]
    pub program_token_account: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
}
#[derive(Accounts)]
pub struct WithdrawFees<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}
#[account]
pub struct EpochAccount {
    pub total_miners: u64,
}
#[derive(Accounts)]
#[instruction(epoch: u64)]
pub struct NewEpoch<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"global"],
        bump,
    )]
    pub global_account: Account<'info, GlobalDataAccount>,
    #[account(
        init,
        payer = signer,
        seeds = [b"epoch", epoch.to_le_bytes().as_ref()],
        bump,
        space = 8 + 8,
    )]
    pub epoch_account: Account<'info, EpochAccount>,
    pub system_program: Program<'info, System>,
}
#[account]
pub struct MineAccount {}
#[derive(Accounts)]
#[instruction(epoch: u64)]
pub struct Mine<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        init,
        seeds = [b"mine", signer.key().as_ref(), epoch.to_le_bytes().as_ref()],
        bump,
        payer = signer,
        space = 8,
    )]
    pub mine_account: Account<'info, MineAccount>,
    #[account(
        mut,
        seeds = [b"epoch", epoch.to_le_bytes().as_ref()],
        bump,
    )]
    pub epoch_account: Account<'info, EpochAccount>,
    #[account(
        seeds = [b"global"],
        bump
    )]
    pub global_account: Account<'info, GlobalDataAccount>,
    #[account(
        mut,
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    pub system_program: Program<'info, System>,
}
#[derive(Accounts)]
#[instruction(epoch: u64)]
pub struct Claim<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"mine", signer.key().as_ref(), epoch.to_le_bytes().as_ref()],
        bump,
        close = signer,
    )]
    pub mine_account: Account<'info, MineAccount>,
    pub signer_token_account: Account<'info, TokenAccount>,
    #[account(
        mut,
        seeds = [b"token_account"],
        bump,
    )]
    pub program_token_account: Account<'info, TokenAccount>,
    #[account(
        seeds = [b"auth"],
        bump,
    )]
    /// CHECK: 
    pub program_authority: AccountInfo<'info>,
    #[account(
        seeds = [b"epoch", epoch.to_le_bytes().as_ref()],
        bump,
    )]
    pub epoch_account: Account<'info, EpochAccount>,
    #[account(
        seeds = [b"global"],
        bump,
    )]
    pub global_account: Account<'info, GlobalDataAccount>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}


