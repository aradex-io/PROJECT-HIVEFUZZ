/// Input mutation engine — applies mutation operators to fuzz inputs.
use rand::Rng;

use super::MutationType;

/// Apply a mutation operator to an input buffer, returning the mutated result.
pub fn apply_mutation(input: &[u8], mutation: MutationType) -> Vec<u8> {
    if input.is_empty() {
        // Can't mutate empty input — return a single random byte
        return vec![rand::thread_rng().r#gen()];
    }

    let mut data = input.to_vec();
    let mut rng = rand::thread_rng();

    match mutation {
        MutationType::BitFlip1 => {
            let pos = rng.r#gen_range(0..data.len());
            let bit = rng.r#gen_range(0..8u8);
            data[pos] ^= 1 << bit;
        }
        MutationType::BitFlip2 => {
            let pos = rng.r#gen_range(0..data.len());
            let bit = rng.r#gen_range(0..7u8);
            data[pos] ^= 3 << bit;
        }
        MutationType::BitFlip4 => {
            let pos = rng.r#gen_range(0..data.len());
            let bit = rng.r#gen_range(0..5u8);
            data[pos] ^= 0x0F << bit;
        }
        MutationType::ByteFlip1 => {
            let pos = rng.r#gen_range(0..data.len());
            data[pos] ^= 0xFF;
        }
        MutationType::ByteFlip2 => {
            if data.len() >= 2 {
                let pos = rng.r#gen_range(0..data.len() - 1);
                data[pos] ^= 0xFF;
                data[pos + 1] ^= 0xFF;
            } else {
                data[0] ^= 0xFF;
            }
        }
        MutationType::ByteFlip4 => {
            if data.len() >= 4 {
                let pos = rng.r#gen_range(0..data.len() - 3);
                for i in 0..4 {
                    data[pos + i] ^= 0xFF;
                }
            } else {
                for byte in data.iter_mut() {
                    *byte ^= 0xFF;
                }
            }
        }
        MutationType::ArithAdd8 => {
            let pos = rng.r#gen_range(0..data.len());
            let val = rng.r#gen_range(1..=35u8);
            data[pos] = data[pos].wrapping_add(val);
        }
        MutationType::ArithSub8 => {
            let pos = rng.r#gen_range(0..data.len());
            let val = rng.r#gen_range(1..=35u8);
            data[pos] = data[pos].wrapping_sub(val);
        }
        MutationType::ArithAdd16 => {
            if data.len() >= 2 {
                let pos = rng.r#gen_range(0..data.len() - 1);
                let val = rng.r#gen_range(1..=35u16);
                let current = u16::from_le_bytes([data[pos], data[pos + 1]]);
                let new_val = current.wrapping_add(val);
                let bytes = new_val.to_le_bytes();
                data[pos] = bytes[0];
                data[pos + 1] = bytes[1];
            } else {
                data[0] = data[0].wrapping_add(rng.r#gen_range(1..=35));
            }
        }
        MutationType::ArithSub16 => {
            if data.len() >= 2 {
                let pos = rng.r#gen_range(0..data.len() - 1);
                let val = rng.r#gen_range(1..=35u16);
                let current = u16::from_le_bytes([data[pos], data[pos + 1]]);
                let new_val = current.wrapping_sub(val);
                let bytes = new_val.to_le_bytes();
                data[pos] = bytes[0];
                data[pos + 1] = bytes[1];
            } else {
                data[0] = data[0].wrapping_sub(rng.r#gen_range(1..=35));
            }
        }
        MutationType::ArithAdd32 => {
            if data.len() >= 4 {
                let pos = rng.r#gen_range(0..data.len() - 3);
                let val = rng.r#gen_range(1..=35u32);
                let current = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
                let new_val = current.wrapping_add(val);
                let bytes = new_val.to_le_bytes();
                data[pos..pos+4].copy_from_slice(&bytes);
            } else {
                data[0] = data[0].wrapping_add(rng.r#gen_range(1..=35));
            }
        }
        MutationType::ArithSub32 => {
            if data.len() >= 4 {
                let pos = rng.r#gen_range(0..data.len() - 3);
                let val = rng.r#gen_range(1..=35u32);
                let current = u32::from_le_bytes([data[pos], data[pos+1], data[pos+2], data[pos+3]]);
                let new_val = current.wrapping_sub(val);
                let bytes = new_val.to_le_bytes();
                data[pos..pos+4].copy_from_slice(&bytes);
            } else {
                data[0] = data[0].wrapping_sub(rng.r#gen_range(1..=35));
            }
        }
        MutationType::InterestingValue8 => {
            let interesting: &[u8] = &[0, 1, 16, 32, 64, 100, 127, 128, 255];
            let pos = rng.r#gen_range(0..data.len());
            data[pos] = interesting[rng.r#gen_range(0..interesting.len())];
        }
        MutationType::InterestingValue16 => {
            let interesting: &[u16] = &[0, 128, 255, 256, 512, 1000, 1024, 4096, 32767, 65535];
            if data.len() >= 2 {
                let pos = rng.r#gen_range(0..data.len() - 1);
                let val = interesting[rng.r#gen_range(0..interesting.len())];
                let bytes = val.to_le_bytes();
                data[pos] = bytes[0];
                data[pos + 1] = bytes[1];
            } else {
                data[0] = rng.r#gen();
            }
        }
        MutationType::InterestingValue32 => {
            let interesting: &[u32] = &[0, 256, 65535, 65536, 100_663_045, 2_147_483_647, 4_294_967_295];
            if data.len() >= 4 {
                let pos = rng.r#gen_range(0..data.len() - 3);
                let val = interesting[rng.r#gen_range(0..interesting.len())];
                let bytes = val.to_le_bytes();
                data[pos..pos+4].copy_from_slice(&bytes);
            } else {
                data[0] = rng.r#gen();
            }
        }
        MutationType::RandomByte => {
            let pos = rng.r#gen_range(0..data.len());
            data[pos] = rng.r#gen();
        }
        MutationType::DeleteBlock => {
            if data.len() > 1 {
                let max_len = (data.len() / 4).max(1).min(32);
                let block_len = rng.r#gen_range(1..=max_len);
                let pos = rng.r#gen_range(0..data.len());
                let end = (pos + block_len).min(data.len());
                data.drain(pos..end);
                if data.is_empty() {
                    data.push(0);
                }
            }
        }
        MutationType::InsertBlock => {
            let max_len = 32.min(data.len().max(4));
            let block_len = rng.r#gen_range(1..=max_len);
            let pos = rng.r#gen_range(0..=data.len());
            let block: Vec<u8> = (0..block_len).map(|_| rng.r#gen()).collect();
            data.splice(pos..pos, block);
        }
        MutationType::OverwriteBlock => {
            if data.len() > 1 {
                let max_len = (data.len() / 4).max(1).min(32);
                let block_len = rng.r#gen_range(1..=max_len);
                let pos = rng.r#gen_range(0..data.len());
                for i in 0..block_len {
                    if pos + i < data.len() {
                        data[pos + i] = rng.r#gen();
                    }
                }
            }
        }
        MutationType::Splice => {
            // Splice with itself (cross-over needs a second input from caller)
            if data.len() > 2 {
                let mid = rng.r#gen_range(1..data.len() - 1);
                let other_mid = rng.r#gen_range(1..data.len() - 1);
                let mut spliced = data[..mid].to_vec();
                spliced.extend_from_slice(&data[other_mid..]);
                data = spliced;
            }
        }
        MutationType::DictionaryInsert | MutationType::DictionaryOverwrite => {
            // Dictionary mutations need a dictionary — fall back to random byte
            let pos = rng.r#gen_range(0..data.len());
            data[pos] = rng.r#gen();
        }
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_mutations_produce_output() {
        let input = b"AAAA BBBB CCCC DDDD";
        for mutation_type in MutationType::all() {
            let result = apply_mutation(input, *mutation_type);
            assert!(!result.is_empty(), "Mutation {:?} produced empty output", mutation_type);
        }
    }

    #[test]
    fn test_mutations_modify_input() {
        let input = b"AAAAAAAAAAAAAAAA"; // 16 bytes
        let mut modified_count = 0;
        // Run each mutation type many times — stochastic, but should modify often
        for mutation_type in MutationType::all() {
            for _ in 0..10 {
                let result = apply_mutation(input, *mutation_type);
                if result != input {
                    modified_count += 1;
                    break;
                }
            }
        }
        // At least most mutation types should modify the input
        assert!(
            modified_count >= MutationType::all().len() - 1,
            "Only {}/{} mutation types modified the input",
            modified_count,
            MutationType::all().len()
        );
    }

    #[test]
    fn test_empty_input_mutation() {
        let input = b"";
        for mutation_type in MutationType::all() {
            let result = apply_mutation(input, *mutation_type);
            assert!(!result.is_empty(), "Mutation {:?} produced empty output from empty input", mutation_type);
        }
    }

    #[test]
    fn test_single_byte_mutation() {
        let input = b"X";
        for mutation_type in MutationType::all() {
            let result = apply_mutation(input, *mutation_type);
            assert!(!result.is_empty(), "Mutation {:?} produced empty output from single byte", mutation_type);
        }
    }
}
