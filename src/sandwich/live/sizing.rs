use super::router::Victim;
use crate::arbitrage::state::PoolState;
use crate::common::constants::WETH;
use alloy::primitives::U256;

#[derive(Debug, Clone)]
pub struct Plan {
    pub front_amount: U256,
    pub victim_out: U256,
    pub profit_weth: U256,
    pub back_amount: U256,
    pub front_dir_token0_in: bool,
}

const BPS: u64 = 10_000;

fn out_of(rin: U256, rout: U256, amount: U256) -> Option<U256> {
    if rin.is_zero() || rout.is_zero() || amount.is_zero() {
        return None;
    }
    let fee_in = amount.checked_mul(U256::from(997u64))?;
    let num = fee_in.checked_mul(rout)?;
    let den = rin.checked_mul(U256::from(1000u64))?.checked_add(fee_in)?;
    let out = num / den;
    (out > U256::ZERO && out < rout).then_some(out)
}

fn in_for(rin: U256, rout: U256, out: U256) -> Option<U256> {
    if rin.is_zero() || rout.is_zero() || out.is_zero() || out >= rout {
        return None;
    }
    let num = rin.checked_mul(out)?.checked_mul(U256::from(1000u64))?;
    let den = rout.checked_sub(out)?.checked_mul(U256::from(997u64))?;
    Some(num / den + U256::from(1u64))
}

struct Sides {
    weth: U256,
    token: U256,
}

fn sides(pool: &PoolState, victim: &Victim) -> Option<Sides> {
    let _ = victim;
    if pool.token0 == WETH {
        Some(Sides { weth: pool.reserve0, token: pool.reserve1 })
    } else if pool.token1 == WETH {
        Some(Sides { weth: pool.reserve1, token: pool.reserve0 })
    } else {
        None
    }
}

pub fn evaluate(pool: &PoolState, victim: &Victim, a: U256) -> Option<(U256, U256)> {
    let s = sides(pool, victim)?;
    let buys = victim.victim_buys;

    let front_out = if buys {
        out_of(s.weth, s.token, a)?
    } else {
        out_of(s.token, s.weth, a)?
    };
    let (mut weth, mut tok) = if buys {
        (s.weth.checked_add(a)?, s.token.checked_sub(front_out)?)
    } else {
        (s.weth.checked_sub(front_out)?, s.token.checked_add(a)?)
    };

    let (v_out, v_in) = if victim.exact_out {
        let need = if buys {
            in_for(weth, tok, victim.desired_out)?
        } else {
            in_for(tok, weth, victim.desired_out)?
        };
        if need > victim.limit {
            return None;
        }
        (victim.desired_out, need)
    } else {
        let out = if buys {
            out_of(weth, tok, victim.amount_in)?
        } else {
            out_of(tok, weth, victim.amount_in)?
        };
        if out < victim.limit {
            return None;
        }
        (out, victim.amount_in)
    };
    if buys {
        weth = weth.checked_add(v_in)?;
        tok = tok.checked_sub(v_out)?;
    } else {
        tok = tok.checked_add(v_in)?;
        weth = weth.checked_sub(v_out)?;
    }

    let back_out = if buys {
        out_of(tok, weth, front_out)?
    } else {
        out_of(weth, tok, front_out)?
    };

    let profit = if buys {
        back_out.saturating_sub(a)
    } else {
        back_out
            .checked_mul(weth)?
            .checked_div(tok)
            .unwrap_or(U256::ZERO)
            .saturating_sub(a)
    };
    Some((profit, v_out))
}

fn max_front(pool: &PoolState, victim: &Victim, hi: U256) -> U256 {
    let mut lo = U256::ZERO;
    let mut hi = hi;
    for _ in 0..96 {
        if hi <= lo + U256::from(1u64) {
            break;
        }
        let mid = lo + (hi - lo) / U256::from(2u64);
        match evaluate(pool, victim, mid) {
            Some(_) => lo = mid,
            None => hi = mid,
        }
    }
    lo
}

pub fn plan(pool: &PoolState, victim: &Victim, float: U256, max_share_bps: u64) -> Option<Plan> {
    let s = sides(pool, victim)?;
    let reserve_in = if victim.victim_buys { s.weth } else { s.token };

    let float_in_input = if victim.victim_buys {
        float
    } else if s.weth.is_zero() {
        U256::ZERO
    } else {
        float.checked_mul(s.token)?.checked_div(s.weth)?
    };

    let share_cap = reserve_in.checked_mul(U256::from(max_share_bps))? / U256::from(BPS);
    let hard_hi = float_in_input.min(share_cap);
    if hard_hi.is_zero() {
        return None;
    }

    let hi = max_front(pool, victim, hard_hi);
    if hi.is_zero() {
        return None;
    }

    let (mut lo, mut hi) = (U256::ZERO, hi);
    let mut best = (U256::ZERO, U256::ZERO);
    for _ in 0..64 {
        if hi <= lo + U256::from(2u64) {
            break;
        }
        let span = hi - lo;
        let m1 = lo + span / U256::from(3u64);
        let m2 = hi - span / U256::from(3u64);
        let p1 = evaluate(pool, victim, m1).map(|v| v.0).unwrap_or(U256::ZERO);
        let p2 = evaluate(pool, victim, m2).map(|v| v.0).unwrap_or(U256::ZERO);
        if p1 > best.1 {
            best = (m1, p1);
        }
        if p2 > best.1 {
            best = (m2, p2);
        }
        if p1 < p2 {
            lo = m1;
        } else {
            hi = m2;
        }
    }
    if best.1.is_zero() {
        return None;
    }

    let (profit, victim_out) = evaluate(pool, victim, best.0)?;
    let back_amount = if victim.victim_buys {
        out_of(s.weth, s.token, best.0)?
    } else {
        out_of(s.token, s.weth, best.0)?
    };
    Some(Plan {
        front_amount: best.0,
        victim_out,
        profit_weth: profit,
        back_amount,
        front_dir_token0_in: pool.token0 == WETH,
    })
}

pub fn slippage_bps(expected: U256, limit: U256) -> u64 {
    if expected.is_zero() || limit >= expected {
        return 0;
    }
    let gap = expected - limit;
    let bps = gap
        .checked_mul(U256::from(BPS))
        .and_then(|v| v.checked_div(expected))
        .unwrap_or(U256::ZERO);
    bps.try_into().unwrap_or(BPS)
}
