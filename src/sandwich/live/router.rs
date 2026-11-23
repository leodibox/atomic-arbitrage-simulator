use alloy::primitives::{Address, Bytes, U256};
use tracing::info;

const IN_TOKENS: [u8; 4] = [0x38, 0xed, 0x17, 0x39];
const IN_TOKENS_FOT: [u8; 4] = [0x5c, 0x11, 0xd7, 0x95];
const IN_ETH: [u8; 4] = [0x7f, 0xf3, 0x6a, 0xb5];
const IN_ETH_FOT: [u8; 4] = [0xb6, 0xf9, 0xde, 0x95];
const IN_TOKENS_ETH: [u8; 4] = [0x18, 0xcb, 0xaf, 0xe5];
const IN_TOKENS_ETH_FOT: [u8; 4] = [0x79, 0x1a, 0xc9, 0x47];
const OUT_TOKENS: [u8; 4] = [0x88, 0x03, 0xdb, 0xee];
const OUT_ETH: [u8; 4] = [0xfb, 0x3b, 0xdb, 0x41];
const UR_EXECUTE_V2: [u8; 4] = [0x35, 0x93, 0x56, 0x4c];
const UR_EXECUTE: [u8; 4] = [0x24, 0x85, 0x6b, 0xc3];
const V2_SWAP_EXACT_IN: u8 = 0x08;

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
    In {
        amount: Option<usize>,
        limit: usize,
        path: usize,
    },
    Out {
        desired: usize,
        max_in: Option<usize>,
        path: usize,
    },
}

fn shape(s: &[u8]) -> Option<Shape> {
    let k: [u8; 4] = s.try_into().ok()?;
    Some(match k {
        IN_TOKENS | IN_TOKENS_FOT => Shape::In {
            amount: Some(0),
            limit: 1,
            path: 2,
        },
        IN_TOKENS_ETH | IN_TOKENS_ETH_FOT => Shape::In {
            amount: Some(0),
            limit: 1,
            path: 2,
        },
        IN_ETH | IN_ETH_FOT => Shape::In {
            amount: None,
            limit: 0,
            path: 1,
        },
        OUT_TOKENS => Shape::Out {
            desired: 0,
            max_in: Some(1),
            path: 2,
        },
        OUT_ETH => Shape::Out {
            desired: 0,
            max_in: None,
            path: 1,
        },
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

pub fn decode<F>(input: &Bytes, value: U256, pair_for: F, weth: Address) -> Option<Victim>
where
    F: Fn(Address, Address) -> Option<(Address, bool)>,
{
    let d = input.as_ref();
    if d.len() < 4 {
        return None;
    }
    let sel: [u8; 4] = d[..4].try_into().ok()?;
    if sel == UR_EXECUTE_V2 || sel == UR_EXECUTE {
        return decode_universal(d, value, &pair_for, weth);
    }

    let sh = shape(&d[..4])?;
    let body = &d[4..];
    let (pi, exact_out, amount_in, limit, desired_out) = match sh {
        Shape::In {
            amount,
            limit,
            path,
        } => (
            path,
            false,
            match amount {
                Some(i) => word(body, i)?,
                None => value,
            },
            word(body, limit)?,
            U256::ZERO,
        ),
        Shape::Out {
            desired,
            max_in,
            path,
        } => (
            path,
            true,
            U256::ZERO,
            match max_in {
                Some(i) => word(body, i)?,
                None => value,
            },
            word(body, desired)?,
        ),
    };
    let Some(path) = path_at(body, pi) else {
        info!("skip: path not 2-token");
        return None;
    };
    let (a, b) = (path[0], path[1]);
    let (token_in, token_out) = if a == weth {
        (a, b)
    } else if b == weth {
        (b, a)
    } else {
        info!("skip: no WETH side");
        return None;
    };
    if token_in == token_out {
        return None;
    }
    let Some((pair, token0_is_main)) = pair_for(token_in, token_out) else {
        info!(%token_in, %token_out, "skip: pair not in universe");
        return None;
    };
    Some(Victim {
        pair,
        token0_is_main,
        victim_buys: a == weth,
        amount_in,
        limit,
        exact_out,
        desired_out,
    })
}

fn decode_universal<F>(d: &[u8], value: U256, pair_for: &F, weth: Address) -> Option<Victim>
where
    F: Fn(Address, Address) -> Option<(Address, bool)>,
{
    let body = d.get(4..)?;
    let cmds_off: usize = word(body, 0)?.try_into().ok()?;
    let cmds_len: usize = word(body, cmds_off / 32)?.try_into().ok()?;
    let cmds_at = cmds_off.checked_add(32)?;
    let cmds = body.get(cmds_at..cmds_at.checked_add(cmds_len)?)?;

    let in_off: usize = word(body, 1)?.try_into().ok()?;
    let in_len: usize = word(body, in_off / 32)?.try_into().ok()?;
    if in_len != cmds_len {
        return None;
    }
    let in_base = in_off.checked_add(32)?;

    let mut best: Option<Victim> = None;
    for (i, cmd) in cmds.iter().enumerate() {
        if cmd & 0x3f != V2_SWAP_EXACT_IN {
            continue;
        }
        let rel: usize = word(body, in_base / 32 + i)?.try_into().ok()?;
        let at = in_base.checked_add(rel)?;
        let len: usize = word(body, at / 32)?.try_into().ok()?;
        let start = at.checked_add(32)?;
        let Some(payload) = body.get(start..start.checked_add(len)?) else {
            continue;
        };
        let Some(v) = decode_universal_swap(payload, value, pair_for, weth) else {
            continue;
        };
        if best
            .as_ref()
            .map(|b| v.amount_in > b.amount_in)
            .unwrap_or(true)
        {
            best = Some(v);
        }
    }
    best
}

fn decode_universal_swap<F>(p: &[u8], value: U256, pair_for: &F, weth: Address) -> Option<Victim>
where
    F: Fn(Address, Address) -> Option<(Address, bool)>,
{
    let contract_balance = U256::from_limbs([0, 0, 0, 1u64 << 63]);
    let mut amount_in = word(p, 1)?;
    let limit = word(p, 2)?;
    if amount_in == contract_balance {
        if value.is_zero() {
            return None;
        }
        amount_in = value;
    }
    if amount_in.is_zero() {
        return None;
    }
    let Some(path) = path_at(p, 3) else {
        info!("skip: path not 2-token");
        return None;
    };
    let (a, b) = (path[0], path[1]);
    let (token_in, token_out) = if a == weth {
        (a, b)
    } else if b == weth {
        (b, a)
    } else {
        info!("skip: no WETH side");
        return None;
    };
    if token_in == token_out {
        return None;
    }
    let Some((pair, token0_is_main)) = pair_for(token_in, token_out) else {
        info!(%token_in, %token_out, "skip: pair not in universe");
        return None;
    };
    Some(Victim {
        pair,
        token0_is_main,
        victim_buys: a == weth,
        amount_in,
        limit,
        exact_out: false,
        desired_out: U256::ZERO,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::primitives::address;

    fn w() -> Address {
        address!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")
    }
    fn t() -> Address {
        address!("A0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")
    }
    fn p() -> Address {
        address!("B4e16d0168e52d35CaCD2c6185b44281Ec28C9Dc")
    }
    fn lookup(a: Address, b: Address) -> Option<(Address, bool)> {
        let _ = (a, b);
        Some((p(), true))
    }

    #[test]
    fn uses_msg_value_for_eth_in_swap() {
        let mut out = Vec::new();
        out.extend_from_slice(&IN_ETH);
        out.extend_from_slice(&U256::from(900u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(128u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(0u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(0u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(2u64).to_be_bytes::<32>());
        for a in [w(), t()] {
            out.extend_from_slice(&[0u8; 12]);
            out.extend_from_slice(a.as_slice());
        }
        let v = decode(&Bytes::from(out), U256::from(1_000u64), lookup, w()).unwrap();
        assert_eq!(v.amount_in, U256::from(1_000u64));
        assert_eq!(v.limit, U256::from(900u64));
        assert!(v.victim_buys);
    }

    #[test]
    fn reads_amount_from_calldata_for_token_in_swap() {
        let mut out = Vec::new();
        out.extend_from_slice(&IN_TOKENS);
        out.extend_from_slice(&U256::from(1_000u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(900u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(160u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(0u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(0u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(2u64).to_be_bytes::<32>());
        for a in [t(), w()] {
            out.extend_from_slice(&[0u8; 12]);
            out.extend_from_slice(a.as_slice());
        }
        let v = decode(&Bytes::from(out), U256::ZERO, lookup, w()).unwrap();
        assert_eq!(v.amount_in, U256::from(1_000u64));
        assert!(!v.victim_buys);
    }
    #[test]
    fn uses_msg_value_for_eth_out_swap() {
        let mut out = Vec::new();
        out.extend_from_slice(&OUT_ETH);
        out.extend_from_slice(&U256::from(1_000u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(128u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(0u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(0u64).to_be_bytes::<32>());
        out.extend_from_slice(&U256::from(2u64).to_be_bytes::<32>());
        for a in [w(), t()] {
            out.extend_from_slice(&[0u8; 12]);
            out.extend_from_slice(a.as_slice());
        }
        let v = decode(&Bytes::from(out), U256::from(500u64), lookup, w()).unwrap();
        assert!(v.exact_out);
        assert_eq!(v.limit, U256::from(500u64));
        assert_eq!(v.desired_out, U256::from(1_000u64));
    }

    fn ur_execute(commands: &[u8], inputs: &[Vec<u8>]) -> Vec<u8> {
        let mut body = Vec::new();
        let cmds_total = 32 + (commands.len() + 31) / 32 * 32;
        body.extend_from_slice(&U256::from(0x60u64).to_be_bytes::<32>());
        body.extend_from_slice(&U256::from((0x60 + cmds_total) as u64).to_be_bytes::<32>());
        body.extend_from_slice(&U256::from(0u64).to_be_bytes::<32>());
        body.extend_from_slice(&U256::from(commands.len() as u64).to_be_bytes::<32>());
        body.extend_from_slice(commands);
        body.resize(0x60 + cmds_total, 0);
        body.extend_from_slice(&U256::from(inputs.len() as u64).to_be_bytes::<32>());
        let mut rel = inputs.len() * 32;
        for inp in inputs {
            body.extend_from_slice(&U256::from(rel as u64).to_be_bytes::<32>());
            rel += 32 + (inp.len() + 31) / 32 * 32;
        }
        for inp in inputs {
            body.extend_from_slice(&U256::from(inp.len() as u64).to_be_bytes::<32>());
            body.extend_from_slice(inp);
            while body.len() % 32 != 0 {
                body.push(0);
            }
        }
        let mut out = Vec::new();
        out.extend_from_slice(&UR_EXECUTE_V2);
        out.extend_from_slice(&body);
        out
    }

    fn ur_v2_swap(
        amount_in: U256,
        amount_out_min: U256,
        path: [Address; 2],
        payer_is_user: bool,
    ) -> Vec<u8> {
        let mut p = Vec::new();
        p.extend_from_slice(&[0u8; 32]);
        p.extend_from_slice(&amount_in.to_be_bytes::<32>());
        p.extend_from_slice(&amount_out_min.to_be_bytes::<32>());
        p.extend_from_slice(&U256::from(0xa0u64).to_be_bytes::<32>());
        p.extend_from_slice(&U256::from(payer_is_user as u64).to_be_bytes::<32>());
        p.extend_from_slice(&U256::from(2u64).to_be_bytes::<32>());
        for a in path {
            p.extend_from_slice(&[0u8; 12]);
            p.extend_from_slice(a.as_slice());
        }
        p
    }

    #[test]
    fn decodes_universal_router_v2_swap() {
        let cd = ur_execute(
            &[V2_SWAP_EXACT_IN],
            &[ur_v2_swap(
                U256::from(1_000u64),
                U256::from(900u64),
                [w(), t()],
                true,
            )],
        );
        let v = decode(&Bytes::from(cd), U256::ZERO, lookup, w()).unwrap();
        assert_eq!(v.amount_in, U256::from(1_000u64));
        assert_eq!(v.limit, U256::from(900u64));
        assert!(v.victim_buys);
        assert!(!v.exact_out);
    }

    #[test]
    fn decodes_universal_swap_behind_revert_flag() {
        let cd = ur_execute(
            &[V2_SWAP_EXACT_IN | 0x80],
            &[ur_v2_swap(U256::from(7u64), U256::ZERO, [t(), w()], true)],
        );
        let v = decode(&Bytes::from(cd), U256::ZERO, lookup, w()).unwrap();
        assert_eq!(v.amount_in, U256::from(7u64));
        assert!(!v.victim_buys);
    }

    #[test]
    fn ignores_universal_router_v3_only() {
        let cd = ur_execute(&[0x00], &[vec![0u8; 32]]);
        assert!(decode(&Bytes::from(cd), U256::ZERO, lookup, w()).is_none());
    }

    #[test]
    fn picks_the_largest_universal_swap() {
        let cd = ur_execute(
            &[V2_SWAP_EXACT_IN, V2_SWAP_EXACT_IN],
            &[
                ur_v2_swap(U256::from(5u64), U256::ZERO, [w(), t()], true),
                ur_v2_swap(U256::from(50_000u64), U256::ZERO, [w(), t()], true),
            ],
        );
        let v = decode(&Bytes::from(cd), U256::ZERO, lookup, w()).unwrap();
        assert_eq!(v.amount_in, U256::from(50_000u64));
    }

    #[test]
    fn universal_swap_falls_back_to_msg_value_on_contract_balance() {
        let sentinel = U256::from_limbs([0, 0, 0, 1u64 << 63]);
        let cd = ur_execute(
            &[V2_SWAP_EXACT_IN],
            &[ur_v2_swap(sentinel, U256::ZERO, [w(), t()], false)],
        );
        let v = decode(&Bytes::from(cd), U256::from(1_234u64), lookup, w()).unwrap();
        assert_eq!(v.amount_in, U256::from(1_234u64));
    }

    #[test]
    fn ignores_truncated_universal_calldata() {
        let cd = ur_execute(
            &[V2_SWAP_EXACT_IN],
            &[ur_v2_swap(
                U256::from(1_000u64),
                U256::ZERO,
                [w(), t()],
                true,
            )],
        );
        let cut = cd[..cd.len() - 8].to_vec();
        assert!(decode(&Bytes::from(cut), U256::ZERO, lookup, w()).is_none());
    }
}
