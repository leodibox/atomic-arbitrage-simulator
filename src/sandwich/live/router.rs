use alloy::primitives::{Address, Bytes, U256};

const IN_TOKENS: [u8; 4] = [0x38, 0xed, 0x17, 0x39];
const IN_TOKENS_FOT: [u8; 4] = [0x5c, 0x11, 0xd7, 0x95];
const IN_ETH: [u8; 4] = [0x7f, 0xf3, 0x6a, 0xb5];
const IN_ETH_FOT: [u8; 4] = [0xb6, 0xf9, 0xde, 0x95];
const IN_TOKENS_ETH: [u8; 4] = [0x18, 0xcb, 0xaf, 0xe5];
const IN_TOKENS_ETH_FOT: [u8; 4] = [0x79, 0x1a, 0xc9, 0x47];
const OUT_TOKENS: [u8; 4] = [0x88, 0x03, 0xdb, 0xee];
const OUT_ETH: [u8; 4] = [0xfb, 0x3b, 0xdb, 0x41];

#[derive(Debug, Clone)]
pub struct Victim {
    pub pair: Address,
    pub token0_is_main: bool,
    pub victim_buys: bool,
    pub amount_in: U256,
    pub limit: U256,
    pub exact_out: bool,
    pub desired_out: U256,
}

#[derive(Debug, Clone, Copy)]
enum Shape {
    In { amount: usize, limit: usize, path: usize },
    Out { desired: usize, max_in: usize, path: usize },
}

fn shape(s: &[u8]) -> Option<Shape> {
    let k: [u8; 4] = s.try_into().ok()?;
    Some(match k {
        IN_TOKENS | IN_TOKENS_FOT => Shape::In { amount: 0, limit: 1, path: 2 },
        IN_TOKENS_ETH | IN_TOKENS_ETH_FOT => Shape::In { amount: 0, limit: 1, path: 2 },
        IN_ETH | IN_ETH_FOT => Shape::In { amount: 0, limit: 0, path: 1 },
        OUT_TOKENS => Shape::Out { desired: 0, max_in: 1, path: 2 },
        OUT_ETH => Shape::Out { desired: 0, max_in: 0, path: 1 },
        _ => return None,
    })
}

fn word(d: &[u8], i: usize) -> Option<U256> {
    let s = i.checked_mul(32)?;
    Some(U256::from_be_slice(d.get(s..s + 32)?))
}

fn path_at(d: &[u8], head: usize) -> Option<Vec<Address>> {
    let off: usize = word(d, head)?.try_into().ok()?;
    let len: usize = word(d, off / 32)?.try_into().ok()?;
    if len != 2 {
        return None;
    }
    let mut out = Vec::with_capacity(2);
    for i in 0..2usize {
        let at = off.checked_add(32)?.checked_add(i * 32)?;
        let s = d.get(at..at + 32)?;
        out.push(Address::from_slice(&s[12..32]));
    }
    Some(out)
}

pub fn decode<F>(input: &Bytes, pair_for: F, weth: Address) -> Option<Victim>
where
    F: Fn(Address, Address) -> Option<(Address, bool)>,
{
    let d = input.as_ref();
    if d.len() < 4 {
        return None;
    }
    let sh = shape(&d[..4])?;
    let body = &d[4..];
    let (pi, exact_out, amount_in, limit, desired_out) = match sh {
        Shape::In { amount, limit, path } => {
            (path, false, word(body, amount)?, word(body, limit)?, U256::ZERO)
        }
        Shape::Out { desired, max_in, path } => {
            (path, true, U256::ZERO, word(body, max_in)?, word(body, desired)?)
        }
    };
    let path = path_at(body, pi)?;
    let (a, b) = (path[0], path[1]);
    let (token_in, token_out) = if a == weth {
        (a, b)
    } else if b == weth {
        (b, a)
    } else {
        return None;
    };
    if token_in == token_out {
        return None;
    }
    let (pair, token0_is_main) = pair_for(token_in, token_out)?;
    Some(Victim {
        pair,
        token0_is_main,
        victim_buys: token_in == weth,
        amount_in,
        limit,
        exact_out,
        desired_out,
    })
}
